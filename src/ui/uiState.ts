// src/ui/uiState.ts
// Domain state object #5: UIState
// Outliner view mode, shading mode, backdrop theme, pivot markers, modals, toasts.

export type ShadingMode = "pbr" | "clay" | "wireframe" | "normals";

export interface Toast {
  id: number;
  title: string;
  message: string;
  visible: boolean;
}

export interface UIState {
  outlinerViewMode: OutlinerView;
  shadingMode: ShadingMode;
  lightBackdrop: boolean;
  showJointPivotMarkers: boolean;
  referencePlanesVisible: boolean;
  modals: {
    blenderScript: boolean;
    importCharacter: boolean;
    rigReview: boolean;
    topologyNumbers: boolean;
    headerGrepDropdown: boolean;
  };
  toasts: Toast[];
  activeAnimBadge: string;
  projBlendLabel: string;
  bumpValLabel: string;
  wasdModeText: string;
  turntableText: string;
}

export type OutlinerView = "groups" | "parts26";

export function createInitialUIState(): UIState {
  return {
    outlinerViewMode: "groups",
    shadingMode: "pbr",
    lightBackdrop: true,
    showJointPivotMarkers: false,
    referencePlanesVisible: false,
    modals: {
      blenderScript: false,
      importCharacter: false,
      rigReview: false,
      topologyNumbers: false,
      headerGrepDropdown: false,
    },
    toasts: [],
    activeAnimBadge: "IDLE",
    projBlendLabel: "0% (PBR)",
    bumpValLabel: "100%",
    wasdModeText: "WASD Move: READY",
    turntableText: "Auto-Turntable: OFF",
  };
}

export function addToast(state: UIState, title: string, message: string): number {
  const id = Date.now();
  state.toasts.push({ id, title, message, visible: true });
  setTimeout(() => {
    const t = state.toasts.find((x) => x.id === id);
    if (t) t.visible = false;
  }, 4200);
  return id;
}

export function removeToast(state: UIState, id: number): void {
  state.toasts = state.toasts.filter((t) => t.id !== id);
}

export function setModal(state: UIState, name: keyof UIState["modals"], open: boolean): void {
  state.modals[name] = open;
}

export function setShadingMode(state: UIState, mode: ShadingMode): void {
  state.shadingMode = mode;
}

export function toggleBackdrop(state: UIState): boolean {
  state.lightBackdrop = !state.lightBackdrop;
  return state.lightBackdrop;
}
