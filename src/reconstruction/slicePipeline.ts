// src/reconstruction/slicePipeline.ts
// Modular 2D->3D slice processing functions extracted from reprocessUploadedImage
// and buildVolumetricMeshFromSlices. Pure functions operating on image data.
// These power the turnaround auto-slicer and the 360 volumetric silhouette builder.

export type Cell = { x0: number; x1: number; y0: number; y1: number };

// Returns whether a pixel matches a background color within tolerance squared.
function pixelMatchesBg(
  rgba: Uint8ClampedArray,
  idx: number,
  bgR: number,
  bgG: number,
  bgB: number,
  threshSq: number
): boolean {
  if (rgba[idx + 3] < 40) return true;
  const dr = rgba[idx] - bgR;
  const dg = rgba[idx + 1] - bgG;
  const db = rgba[idx + 2] - bgB;
  return dr * dr + dg * dg + db * db <= threshSq;
}

// Border-sampled + inset background color estimate.
export function estimateSheetBg(
  rgba: Uint8ClampedArray,
  width: number,
  height: number
): { r: number; g: number; b: number } {
  const samples = [
    [Math.floor(width * 0.05), Math.floor(height * 0.15)],
    [Math.floor(width * 0.50), Math.floor(height * 0.15)],
    [Math.floor(width * 0.95), Math.floor(height * 0.15)],
    [Math.floor(width * 0.05), Math.floor(height * 0.85)],
    [Math.floor(width * 0.95), Math.floor(height * 0.85)],
  ];
  let r = 0, g = 0, b = 0;
  for (const [x, y] of samples) {
    const sx = Math.min(width - 1, Math.max(0, x));
    const sy = Math.min(height - 1, Math.max(0, y));
    const i = (sy * width + sx) * 4;
    r += rgba[i];
    g += rgba[i + 1];
    b += rgba[i + 2];
  }
  const n = samples.length;
  return { r: r / n, g: g / n, b: b / n };
}

// Row-band detection: returns horizontal bands of foreground (separates title banner,
// pose cards, caption text bands).
export function detectRowBands(
  rgba: Uint8ClampedArray,
  width: number,
  height: number,
  bg: { r: number; g: number; b: number },
  tol: number
): Cell[] {
  const threshSq = tol * tol * 3;
  const counts = new Int32Array(height);
  const marginX = Math.floor(width * 0.035);
  const marginY = Math.floor(height * 0.06);
  for (let y = marginY; y < height - marginY; y++) {
    for (let x = marginX; x < width - marginX; x++) {
      const i = (y * width + x) * 4;
      const dr = rgba[i] - bg.r;
      const dg = rgba[i + 1] - bg.g;
      const db = rgba[i + 2] - bg.b;
      if (dr * dr + dg * dg + db * db > threshSq * 0.65) counts[y]++;
    }
  }
  const bands: Cell[] = [];
  let inRow = false;
  let rStart = 0;
  const minRowPixels = Math.max(8, Math.floor(width * 0.05));
  for (let y = 0; y < height; y++) {
    if (counts[y] >= minRowPixels) {
      if (!inRow) { inRow = true; rStart = y; }
    } else if (inRow) {
      if (y - rStart >= height * 0.14) bands.push({ x0: 0, x1: width - 1, y0: rStart, y1: y - 1 });
      inRow = false;
    }
  }
  if (inRow && height - rStart >= height * 0.14) bands.push({ x0: 0, x1: width - 1, y0: rStart, y1: height - 1 });
  if (bands.length === 0) bands.push({ x0: 0, x1: width - 1, y0: 0, y1: height - 1 });
  return bands;
}

// Cell/island detection within a row band (each pose card / turnaround view).
export function detectPoseCells(
  rgba: Uint8ClampedArray,
  width: number,
  height: number,
  band: Cell,
  bg: { r: number; g: number; b: number },
  tol: number
): Cell[] {
  const threshSq = tol * tol * 3;
  const counts = new Int32Array(width);
  const marginX = Math.floor(width * 0.035);
  const effectiveY1 = Math.min(band.y1, Math.floor(height * 0.86));
  for (let y = band.y0; y <= effectiveY1; y++) {
    for (let x = marginX; x < width - marginX; x++) {
      const i = (y * width + x) * 4;
      const dr = rgba[i] - bg.r;
      const dg = rgba[i + 1] - bg.g;
      const db = rgba[i + 2] - bg.b;
      if (dr * dr + dg * dg + db * db > threshSq * 0.65) counts[x]++;
    }
  }
  const rawIslands: Cell[] = [];
  let inIsl = false;
  let startX = 0;
  const minColPixels = Math.max(4, Math.floor((effectiveY1 - band.y0) * 0.05));
  for (let x = 0; x < width; x++) {
    if (counts[x] >= minColPixels) {
      if (!inIsl) { inIsl = true; startX = x; }
    } else if (inIsl) {
      if (x - startX >= width * 0.05) rawIslands.push({ x0: startX, x1: x - 1, y0: band.y0, y1: effectiveY1 });
      inIsl = false;
    }
  }
  if (inIsl && width - startX >= width * 0.05) rawIslands.push({ x0: startX, x1: width - 1, y0: band.y0, y1: effectiveY1 });
  if (rawIslands.length === 0) rawIslands.push({ x0: marginX, x1: width - 1 - marginX, y0: band.y0, y1: effectiveY1 });

  // Split any wide merged horizontal strip at its lowest column-density valleys
  const islands: Cell[] = [];
  const bandH = Math.max(32, effectiveY1 - band.y0);
  for (const isl of rawIslands) {
    const spanW = isl.x1 - isl.x0;
    if (spanW > bandH * 1.35) {
      const nSplit = spanW > width * 0.72 ? 5 : Math.min(6, Math.max(2, Math.round(spanW / (bandH * 0.52))));
      const cuts = [isl.x0];
      const radius = Math.max(6, Math.floor(spanW / (nSplit * 3.2)));
      for (let k = 1; k < nSplit; k++) {
        const ideal = isl.x0 + Math.round((spanW * k) / nSplit);
        const lo = Math.max(cuts[cuts.length - 1] + 12, ideal - radius);
        const hi = Math.min(isl.x1 - 12, ideal + radius);
        let bestX = ideal;
        let bestScore = Number.MAX_SAFE_INTEGER;
        for (let cx = lo; cx <= hi; cx++) {
          if (counts[cx] < bestScore) {
            bestScore = counts[cx];
            bestX = cx;
          }
        }
        cuts.push(bestX);
      }
      cuts.push(isl.x1);
      for (let k = 0; k < cuts.length - 1; k++) {
        islands.push({ x0: cuts[k], x1: cuts[k + 1], y0: isl.y0, y1: isl.y1 });
      }
    } else {
      islands.push(isl);
    }
  }
  return islands;
}

// Border-seeded flood-fill background removal. Interior light/cream fur is never clipped
// because fill starts only from the cell perimeter. Returns the tight foreground bounds.
export function floodFillRemoveBg(
  rgba: Uint8ClampedArray,
  width: number,
  height: number,
  cell: Cell,
  bg: { r: number; g: number; b: number },
  tol: number
): Cell {
  const threshSq = tol * tol * 3;
  const cw = cell.x1 - cell.x0 + 1;
  const ch = cell.y1 - cell.y0 + 1;
  const visited = new Uint8Array(cw * ch);
  const queue: [number, number][] = [];

  const isBg = (gx: number, gy: number) => {
    const i = (gy * width + gx) * 4;
    return pixelMatchesBg(rgba, i, bg.r, bg.g, bg.b, threshSq);
  };

  // seed BFS from the cell perimeter
  for (let lx = 0; lx < cw; lx++) {
    if (isBg(cell.x0 + lx, cell.y0)) { visited[lx] = 1; queue.push([lx, 0]); }
    const bOff = (ch - 1) * cw + lx;
    if (isBg(cell.x0 + lx, cell.y1) && !visited[bOff]) { visited[bOff] = 1; queue.push([lx, ch - 1]); }
  }
  for (let ly = 1; ly < ch - 1; ly++) {
    const lOff = ly * cw;
    if (isBg(cell.x0, cell.y0 + ly) && !visited[lOff]) { visited[lOff] = 1; queue.push([0, ly]); }
    const rOff = ly * cw + (cw - 1);
    if (isBg(cell.x1, cell.y0 + ly) && !visited[rOff]) { visited[rOff] = 1; queue.push([cw - 1, ly]); }
  }

  let head = 0;
  const dirs = [[1, 0], [-1, 0], [0, 1], [0, -1]];
  while (head < queue.length) {
    const [cx, cy] = queue[head++];
    for (const [dx, dy] of dirs) {
      const nx = cx + dx;
      const ny = cy + dy;
      if (nx < 0 || nx >= cw || ny < 0 || ny >= ch) continue;
      const nOff = ny * cw + nx;
      if (visited[nOff]) continue;
      if (isBg(cell.x0 + nx, cell.y0 + ny)) {
        visited[nOff] = 1;
        queue.push([nx, ny]);
      }
    }
  }

  // apply alpha + compute tight bounds
  let tMinX = cell.x1, tMaxX = cell.x0, tMinY = cell.y1, tMaxY = cell.y0;
  for (let y = cell.y0; y <= cell.y1; y++) {
    const ly = y - cell.y0;
    for (let x = cell.x0; x <= cell.x1; x++) {
      const lx = x - cell.x0;
      const i = (y * width + x) * 4;
      if (visited[ly * cw + lx] || rgba[i + 3] < 40) {
        rgba[i + 3] = 0;
      } else {
        rgba[i + 3] = 255;
        if (x < tMinX) tMinX = x;
        if (x > tMaxX) tMaxX = x;
        if (y < tMinY) tMinY = y;
        if (y > tMaxY) tMaxY = y;
      }
    }
  }
  return { x0: tMinX, x1: tMaxX, y0: tMinY, y1: tMaxY };
}

// Crop a cell to a canvas and fit uniformly into outW x outH, anchoring feet near bottom.
// If synthesizeBackFromFront, horizontally mirror (used to build a back view from front).
export function renderNormalizedSlice(
  srcImgData: Uint8ClampedArray | ImageData,
  srcW: number,
  srcH: number,
  bounds: Cell,
  outW: number,
  outH: number,
  synthesizeBackFromFront = false
): HTMLCanvasElement {
  const raw = (srcImgData as ImageData).data ?? (srcImgData as Uint8ClampedArray);
  const c = document.createElement("canvas");
  c.width = outW;
  c.height = outH;
  const ctx = c.getContext("2d")!;

  const cropW = Math.max(1, bounds.x1 - bounds.x0 + 1);
  const cropH = Math.max(1, bounds.y1 - bounds.y0 + 1);
  const tmp = document.createElement("canvas");
  tmp.width = cropW;
  tmp.height = cropH;
  const tCtx = tmp.getContext("2d")!;
  const subData = tCtx.createImageData(cropW, cropH);

  for (let y = 0; y < cropH; y++) {
    for (let x = 0; x < cropW; x++) {
      const srcX = synthesizeBackFromFront ? bounds.x1 - x : bounds.x0 + x;
      const srcY = bounds.y0 + y;
      const sIdx = (srcY * srcW + srcX) * 4;
      const dIdx = (y * cropW + x) * 4;
      subData.data[dIdx] = raw[sIdx];
      subData.data[dIdx + 1] = raw[sIdx + 1];
      subData.data[dIdx + 2] = raw[sIdx + 2];
      subData.data[dIdx + 3] = raw[sIdx + 3];
    }
  }
  tCtx.putImageData(subData, 0, 0);

  const pad = 8;
  const fitScale = Math.min((outW - pad * 2) / cropW, (outH - pad * 2) / cropH);
  const drawW = cropW * fitScale;
  const drawH = cropH * fitScale;
  const drawX = (outW - drawW) * 0.5;
  const drawY = outH - pad - drawH;

  ctx.clearRect(0, 0, outW, outH);
  ctx.drawImage(tmp, drawX, drawY, drawW, drawH);
  return c;
}

// Build a combined front/back texture atlas (512x512) for GLB/WebGL export.
export function buildTextureAtlas(frontCanvas: HTMLCanvasElement, backCanvas: HTMLCanvasElement): HTMLCanvasElement {
  const atlas = document.createElement("canvas");
  atlas.width = 512;
  atlas.height = 512;
  const aCtx = atlas.getContext("2d")!;
  aCtx.fillStyle = "#e8dfd1";
  aCtx.fillRect(0, 0, 512, 512);
  aCtx.drawImage(frontCanvas, 0, 0, 256, 512);
  aCtx.drawImage(backCanvas, 256, 0, 256, 512);
  return atlas;
}
