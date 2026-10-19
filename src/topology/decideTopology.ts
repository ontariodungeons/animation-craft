export function doesRegionNeedBendLoop(
  jointType: "rigid" | "hinge" | "spline",
  maxBendDeg: number
): { needsLoop: boolean; ringCount: number } {
  const absDeg = Math.abs(maxBendDeg);
  if (jointType === "rigid" || absDeg < 5) {
    return { needsLoop: false, ringCount: 1 };
  }
  if (jointType === "spline") {
    return { needsLoop: true, ringCount: Math.min(7, Math.max(3, Math.round(absDeg / 12))) };
  }
  return { needsLoop: true, ringCount: absDeg > 65 ? 5 : 3 };
}

export function insertJointDeformationLoops(
  vertices: Float32Array,
  indices: Uint32Array,
  jointY: number,
  radius: number,
  ringCount = 3
): { vertices: Float32Array; indices: Uint32Array; addedVerts: number; addedLoops: number } {
  const clampedRings = Math.max(1, Math.min(7, Math.floor(ringCount)));
  const radialSegs = 8;
  const bandHalfHeight = Math.max(0.01, radius * 0.35);

  const addedVertCount = clampedRings * radialSegs;
  const addedTriCount = Math.max(1, clampedRings - 1) * radialSegs * 2;

  const baseVertCount = Math.floor(vertices.length / 3);
  const nextVerts = new Float32Array((baseVertCount + addedVertCount) * 3);
  nextVerts.set(vertices, 0);

  for (let r = 0; r < clampedRings; r++) {
    const t = clampedRings === 1 ? 0.5 : r / (clampedRings - 1);
    const y = jointY - bandHalfHeight + t * (2 * bandHalfHeight);
    for (let s = 0; s < radialSegs; s++) {
      const ang = (s / radialSegs) * Math.PI * 2;
      const vIdx = (baseVertCount + r * radialSegs + s) * 3;
      nextVerts[vIdx] = Math.cos(ang) * radius;
      nextVerts[vIdx + 1] = y;
      nextVerts[vIdx + 2] = Math.sin(ang) * radius;
    }
  }

  const nextIndices = new Uint32Array(indices.length + addedTriCount * 3);
  nextIndices.set(indices, 0);
  let ptr = indices.length;

  const ringsForTris = Math.max(2, clampedRings);
  for (let r = 0; r < ringsForTris - 1; r++) {
    const r0 = Math.min(clampedRings - 1, r);
    const r1 = Math.min(clampedRings - 1, r + 1);
    for (let s = 0; s < radialSegs; s++) {
      const sNext = (s + 1) % radialSegs;
      const a = baseVertCount + r0 * radialSegs + s;
      const b = baseVertCount + r0 * radialSegs + sNext;
      const c = baseVertCount + r1 * radialSegs + s;
      const d = baseVertCount + r1 * radialSegs + sNext;
      nextIndices[ptr++] = a;
      nextIndices[ptr++] = b;
      nextIndices[ptr++] = c;
      nextIndices[ptr++] = b;
      nextIndices[ptr++] = d;
      nextIndices[ptr++] = c;
    }
  }

  return {
    vertices: nextVerts,
    indices: nextIndices,
    addedVerts: addedVertCount,
    addedLoops: addedTriCount * 3
  };
}

export function computeMeshTopologyMetrics(
  vertices: Float32Array,
  indices: Uint32Array
): {
  bufferVerts: number;
  weldedVerts: number;
  edges: number;
  boundaryEdges: number;
  tris: number;
  loops: number;
} {
  const bufferVerts = Math.floor(vertices.length / 3);
  const tris = Math.floor(indices.length / 3);
  const loops = tris * 3;

  const vMap = new Map<string, number>();
  const remap = new Int32Array(bufferVerts);
  let weldedVerts = 0;

  for (let i = 0; i < bufferVerts; i++) {
    const kx = Math.round(vertices[i * 3] * 100000);
    const ky = Math.round(vertices[i * 3 + 1] * 100000);
    const kz = Math.round(vertices[i * 3 + 2] * 100000);
    const key = `${kx},${ky},${kz}`;
    let u = vMap.get(key);
    if (u === undefined) {
      u = weldedVerts++;
      vMap.set(key, u);
    }
    remap[i] = u;
  }

  const edgeCounts = new Map<string, number>();
  const addEdge = (a: number, b: number) => {
    if (a === b) return;
    const lo = a < b ? a : b;
    const hi = a < b ? b : a;
    const k = `${lo}_${hi}`;
    edgeCounts.set(k, (edgeCounts.get(k) || 0) + 1);
  };

  for (let i = 0; i < indices.length; i += 3) {
    const a = remap[indices[i]];
    const b = remap[indices[i + 1]];
    const c = remap[indices[i + 2]];
    addEdge(a, b);
    addEdge(b, c);
    addEdge(c, a);
  }

  let boundaryEdges = 0;
  edgeCounts.forEach(cnt => {
    if (cnt === 1) boundaryEdges++;
  });

  return {
    bufferVerts,
    weldedVerts,
    edges: edgeCounts.size,
    boundaryEdges,
    tris,
    loops
  };
}

export function borderFloodFillAlpha(
  rgba: Uint8ClampedArray,
  width: number,
  height: number,
  tolerance = 28
): Uint8ClampedArray {
  const out = new Uint8ClampedArray(rgba);
  const corners = [
    [0, 0],
    [width - 1, 0],
    [0, height - 1],
    [width - 1, height - 1]
  ];
  let bgR = 0, bgG = 0, bgB = 0;
  corners.forEach(([x, y]) => {
    const idx = (y * width + x) * 4;
    bgR += out[idx];
    bgG += out[idx + 1];
    bgB += out[idx + 2];
  });
  bgR /= corners.length;
  bgG /= corners.length;
  bgB /= corners.length;

  const threshSq = tolerance * tolerance * 3;
  const visited = new Uint8Array(width * height);
  const queue: number[] = [];

  const matchesBg = (px: number, py: number) => {
    const idx = (py * width + px) * 4;
    if (out[idx + 3] < 40) return true;
    const dr = out[idx] - bgR;
    const dg = out[idx + 1] - bgG;
    const db = out[idx + 2] - bgB;
    return dr * dr + dg * dg + db * db <= threshSq;
  };

  for (let x = 0; x < width; x++) {
    if (matchesBg(x, 0)) { visited[x] = 1; queue.push(x, 0); }
    const bOff = (height - 1) * width + x;
    if (!visited[bOff] && matchesBg(x, height - 1)) { visited[bOff] = 1; queue.push(x, height - 1); }
  }
  for (let y = 1; y < height - 1; y++) {
    const lOff = y * width;
    if (!visited[lOff] && matchesBg(0, y)) { visited[lOff] = 1; queue.push(0, y); }
    const rOff = y * width + (width - 1);
    if (!visited[rOff] && matchesBg(width - 1, y)) { visited[rOff] = 1; queue.push(width - 1, y); }
  }

  let head = 0;
  const dirs = [[1, 0], [-1, 0], [0, 1], [0, -1]];
  while (head < queue.length) {
    const cx = queue[head++];
    const cy = queue[head++];
    for (let d = 0; d < 4; d++) {
      const nx = cx + dirs[d][0];
      const ny = cy + dirs[d][1];
      if (nx < 0 || nx >= width || ny < 0 || ny >= height) continue;
      const nOff = ny * width + nx;
      if (visited[nOff]) continue;
      if (matchesBg(nx, ny)) {
        visited[nOff] = 1;
        queue.push(nx, ny);
      }
    }
  }

  for (let p = 0; p < width * height; p++) {
    if (visited[p]) {
      out[p * 4 + 3] = 0;
    } else if (out[p * 4 + 3] >= 40) {
      out[p * 4 + 3] = 255;
    }
  }
  return out;
}
