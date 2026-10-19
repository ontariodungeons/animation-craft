// src/export/exportState.ts
// Domain state object #6: ExportState
// Export helpers (GLB/GLTF/OBJ/Python/JSX), download state, toast reporting.
// Extracted from the monolithic exportGLB / exportGLTFJson / exportOBJ /
// getBlenderPythonScript / exportEffectCraftJSX.

export interface ExportState {
  lastExportedFile: string | null;
  isExporting: boolean;
}

export function createInitialExportState(): ExportState {
  return {
    lastExportedFile: null,
    isExporting: false,
  };
}

export function triggerDownloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  setTimeout(() => URL.revokeObjectURL(url), 3000);
}

export function prepareCleanExportClone(exportTarget: any): { prevRotY: number; prevPosY: number } {
  const prevRotY = exportTarget.rotation.y;
  const prevPosY = exportTarget.position.y;
  exportTarget.rotation.y = 0;
  exportTarget.position.y = 0;
  exportTarget.updateMatrixWorld(true);
  return { prevRotY, prevPosY };
}

export function restoreAfterExportClone(
  exportTarget: any,
  state: { prevRotY: number; prevPosY: number }
): void {
  exportTarget.rotation.y = state.prevRotY;
  exportTarget.position.y = state.prevPosY;
  exportTarget.updateMatrixWorld(true);
}
