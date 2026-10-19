// src/reconstruction/fourDirectionAtlas.ts
// Builds a 2x2 4-quadrant texture atlas (512x512) from four turnaround view slices:
//   Q0 = Front  0°   (top-left)
//   Q1 = Right  90°  (top-right)
//   Q2 = Back   180° (bottom-left)
//   Q3 = Left   270° (bottom-right)
// Provides smooth triplanar normal-weighted sampling so all 4 directions map cleanly
// onto the 3D turnaround mesh without smeared sides.

const ATLAS_SIZE = 512;
const QUAD_SIZE = 256;

const QUAD_OFFSETS: Record<string, { qx: number; qy: number }> = {
  front: { qx: 0,   qy: 0 },
  right: { qx: 256, qy: 0 },
  back:  { qx: 0,   qy: 256 },
  left:  { qx: 256, qy: 256 },
};

export interface FourDirectionAtlas {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  sample: (
    nx: number,
    ny: number,
    nz: number,
    xNorm: number,
    yNorm: number,
    zNorm: number
  ) => [number, number, number, number];
}

// Build a 2x2 4-quadrant atlas from four turnaround view images/canvases.
export function buildFourDirectionAtlas(
  front: HTMLImageElement | HTMLCanvasElement,
  rightSide: HTMLImageElement | HTMLCanvasElement,
  back: HTMLImageElement | HTMLCanvasElement,
  leftSide: HTMLImageElement | HTMLCanvasElement
): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = ATLAS_SIZE;
  canvas.height = ATLAS_SIZE;
  const ctx = canvas.getContext("2d")!;
  ctx.clearRect(0, 0, ATLAS_SIZE, ATLAS_SIZE);

  const views: Array<{
    img: HTMLImageElement | HTMLCanvasElement;
    qx: number;
    qy: number;
  }> = [
    { img: front, qx: 0, qy: 0 },       // Q0: Front 0°
    { img: rightSide, qx: 256, qy: 0 }, // Q1: Right 90°
    { img: back, qx: 0, qy: 256 },      // Q2: Back 180°
    { img: leftSide, qx: 256, qy: 256 }, // Q3: Left 270°
  ];

  for (const { img, qx, qy } of views) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(qx, qy, QUAD_SIZE, QUAD_SIZE);
    ctx.clip();
    // 6px edge color dilation inside each quadrant so silhouettes never bleed background
    for (const [dx, dy] of [[-4, 0], [4, 0], [0, -4], [0, 4], [-2, -2], [2, -2], [-2, 2], [2, 2], [0, 0]]) {
      ctx.drawImage(img, qx + dx, qy + dy, QUAD_SIZE, QUAD_SIZE);
    }
    ctx.restore();
  }
  (canvas as any)._cachedPixels = ctx.getImageData(0, 0, ATLAS_SIZE, ATLAS_SIZE).data;
  return canvas;
}

// Sample a single pixel from the atlas at absolute pixel coordinates using the cached pixel buffer.
function sampleAtlasPixel(
  canvas: HTMLCanvasElement,
  px: number,
  py: number
): [number, number, number, number] {
  const x = Math.min(canvas.width - 1, Math.max(0, Math.floor(px)));
  const y = Math.min(canvas.height - 1, Math.max(0, Math.floor(py)));
  let data: Uint8ClampedArray | undefined = (canvas as any)._cachedPixels;
  if (!data) {
    const ctx = canvas.getContext("2d")!;
    data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    (canvas as any)._cachedPixels = data;
  }
  const idx = (y * canvas.width + x) * 4;
  return [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]];
}

// Sample one quadrant of the atlas at normalized UV (0..1) with inward fallback if transparent.
function sampleQuadrant(
  canvas: HTMLCanvasElement,
  quad: { qx: number; qy: number },
  u: number,
  v: number
): [number, number, number, number] {
  const cu = Math.min(0.97, Math.max(0.03, u));
  const cv = Math.min(0.97, Math.max(0.03, v));
  const px = quad.qx + Math.min(QUAD_SIZE - 1, Math.max(0, Math.floor(cu * QUAD_SIZE)));
  const py = quad.qy + Math.min(QUAD_SIZE - 1, Math.max(0, Math.floor((1 - cv) * QUAD_SIZE)));
  const p = sampleAtlasPixel(canvas, px, py);
  if (p[3] > 60) return p;
  // Search toward horizontal center of quadrant (qx + 128) along the same row py
  const midX = quad.qx + (QUAD_SIZE >> 1);
  const step = px < midX ? 4 : -4;
  for (let sx = px; Math.abs(sx - midX) > 4; sx += step) {
    const cand = sampleAtlasPixel(canvas, sx, py);
    if (cand[3] > 60) return cand;
  }
  return [235, 220, 200, 255];
}

// Build a closure that samples the atlas with smooth triplanar normal weights.
// nx, ny, nz: surface normal components
// xNorm, yNorm, zNorm: triplanar texture coordinates (0..1)
export function sampleFourDirectionColor(
  canvas: HTMLCanvasElement,
  nx: number,
  ny: number,
  nz: number,
  xNorm: number,
  yNorm: number,
  zNorm: number
): [number, number, number, number] {
  // Directional weights from surface normal signs (triplanar-style)
  const wFront = Math.max(0, -nz); // facing toward camera (-Z)
  const wBack = Math.max(0, nz); // facing away (+Z)
  const wRight = Math.max(0, nx); // facing right (+X)
  const wLeft = Math.max(0, -nx); // facing left (-X)

  const total = wFront + wBack + wRight + wLeft + 0.0001;
  const invTotal = 1.0 / total;

  // Sample each quadrant using the appropriate triplanar UV
  const cFront = sampleQuadrant(canvas, QUAD_OFFSETS.front, xNorm, yNorm);
  const cRight = sampleQuadrant(canvas, QUAD_OFFSETS.right, zNorm, yNorm);
  const cBack = sampleQuadrant(canvas, QUAD_OFFSETS.back, 1.0 - xNorm, yNorm);
  const cLeft = sampleQuadrant(canvas, QUAD_OFFSETS.left, zNorm, yNorm);

  // Blend by directional weights (smooth triplanar interpolation)
  let r = 0, g = 0, b = 0, a = 0;
  r += cFront[0] * wFront * invTotal;
  g += cFront[1] * wFront * invTotal;
  b += cFront[2] * wFront * invTotal;
  a += cFront[3] * wFront * invTotal;
  r += cRight[0] * wRight * invTotal;
  g += cRight[1] * wRight * invTotal;
  b += cRight[2] * wRight * invTotal;
  a += cRight[3] * wRight * invTotal;
  r += cBack[0] * wBack * invTotal;
  g += cBack[1] * wBack * invTotal;
  b += cBack[2] * wBack * invTotal;
  a += cBack[3] * wBack * invTotal;
  r += cLeft[0] * wLeft * invTotal;
  g += cLeft[1] * wLeft * invTotal;
  b += cLeft[2] * wLeft * invTotal;
  a += cLeft[3] * wLeft * invTotal;

  return [r, g, b, a];
}

// Convenience: build atlas + sampler in one call.
export function buildFourDirectionAtlasWithSampler(
  front: HTMLImageElement | HTMLCanvasElement,
  rightSide: HTMLImageElement | HTMLCanvasElement,
  back: HTMLImageElement | HTMLCanvasElement,
  leftSide: HTMLImageElement | HTMLCanvasElement
): FourDirectionAtlas {
  const canvas = buildFourDirectionAtlas(front, rightSide, back, leftSide);
  const ctx = canvas.getContext("2d")!;
  return {
    canvas,
    ctx,
    sample: (nx, ny, nz, xNorm, yNorm, zNorm) =>
      sampleFourDirectionColor(canvas, nx, ny, nz, xNorm, yNorm, zNorm),
  };
}
