// src/animation/threeAndP5PuppetAnimator.ts
// Standalone Three.js 2.5D layered cutout / pose-sheet animator + p5.js 2D puppet
// / pose-cycle animator. Works with ANY character turnaround sheet, pose sheet,
// single image, or exports/vault_slices/<char_id>/ & pose_and_part_cutter.py manifests.
// Requires NO Blender (Track A: Web-Native 2D / 2.5D Animation).

import * as THREE from "three";
import type { PoseTransforms } from "./proceduralAnimation";
import type { SliderValues } from "./animationState";
import {
  estimateSheetBg,
  detectRowBands,
  detectPoseCells,
  floodFillRemoveBg,
  renderNormalizedSlice,
} from "../reconstruction/slicePipeline";

export function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`Failed to load image: ${url}`));
    img.src = url;
  });
}

export const TURNAROUND_VIEW_SPECS = [
  { view: "front", deg: 0, candidates: ["view_01_0deg.png", "frames/00_front.png", "view_0.png"] },
  { view: "threeQuarterRight", deg: 45, candidates: ["view_02_45deg.png", "frames/01_quarter_front.png", "view_1.png"] },
  { view: "side", deg: 90, candidates: ["view_03_90deg.png", "frames/02_side.png", "view_2.png"] },
  { view: "threeQuarterLeft", deg: -45, candidates: ["view_04_315deg.png", "frames/03_opposite_quarter.png", "view_3.png"] },
  { view: "back", deg: 180, candidates: ["view_05_180deg.png", "frames/04_back.png", "view_4.png"] },
];

export interface CutoutPartSpec {
  id:
    | "ears"
    | "ear_l"
    | "ear_r"
    | "head"
    | "torso"
    | "arm_l"
    | "arm_r"
    | "leg_l"
    | "leg_r"
    | "foot_l"
    | "foot_r"
    | string;
  cropNorm: [number, number, number, number]; // [x0, y0, x1, y1] normalized 0..1 on front sprite
  pivotNorm: [number, number];                // [px, py] normalized 0..1 within full sprite
  zOffset: number;
  parent?: string;
}

// Default anatomical cutout regions for auto-slicing a front sprite when explicit parts/*.png are absent
export const DEFAULT_AUTO_PARTS: CutoutPartSpec[] = [
  { id: "leg_r",  cropNorm: [0.18, 0.66, 0.49, 0.88], pivotNorm: [0.43, 0.68], zOffset: -0.018, parent: "root" },
  { id: "foot_r", cropNorm: [0.14, 0.82, 0.49, 1.00], pivotNorm: [0.39, 0.83], zOffset: -0.015, parent: "leg_r" },
  { id: "leg_l",  cropNorm: [0.50, 0.66, 0.82, 0.88], pivotNorm: [0.57, 0.68], zOffset: -0.012, parent: "root" },
  { id: "foot_l", cropNorm: [0.50, 0.82, 0.86, 1.00], pivotNorm: [0.61, 0.83], zOffset: -0.009, parent: "leg_l" },
  { id: "arm_r",  cropNorm: [0.02, 0.45, 0.30, 0.76], pivotNorm: [0.33, 0.48], zOffset: -0.006, parent: "torso" },
  { id: "torso",  cropNorm: [0.24, 0.43, 0.76, 0.74], pivotNorm: [0.50, 0.66], zOffset: 0.0,    parent: "root" },
  { id: "ear_r",  cropNorm: [0.12, 0.00, 0.50, 0.24], pivotNorm: [0.43, 0.22], zOffset: 0.016,  parent: "head" },
  { id: "ear_l",  cropNorm: [0.50, 0.00, 0.88, 0.24], pivotNorm: [0.55, 0.22], zOffset: 0.019,  parent: "head" },
  { id: "head",   cropNorm: [0.16, 0.18, 0.84, 0.47], pivotNorm: [0.50, 0.43], zOffset: 0.022,  parent: "torso" },
  { id: "arm_l",  cropNorm: [0.70, 0.45, 0.98, 0.76], pivotNorm: [0.67, 0.48], zOffset: 0.028,  parent: "torso" },
];

export interface AnimatorOptions {
  basePath?: string;
  fov?: number;
  near?: number;
  far?: number;
  cameraZ?: number;
  cardHeight?: number;
  backgroundColor?: number;
}

function cropCanvasRegion(
  src: HTMLImageElement | HTMLCanvasElement,
  x0Norm: number,
  y0Norm: number,
  x1Norm: number,
  y1Norm: number
): HTMLCanvasElement {
  const sw = src.width || 512;
  const sh = src.height || 512;
  const c = document.createElement("canvas");
  c.width = sw;
  c.height = sh;
  const ctx = c.getContext("2d")!;
  const sx = Math.floor(x0Norm * sw);
  const sy = Math.floor(y0Norm * sh);
  const cw = Math.max(1, Math.floor((x1Norm - x0Norm) * sw));
  const ch = Math.max(1, Math.floor((y1Norm - y0Norm) * sh));
  ctx.drawImage(src, sx, sy, cw, ch, sx, sy, cw, ch);
  return c;
}

// ---------------------------------------------------------------------------
// Three.js 2.5D layered cutout animator (Mesh planes with proper 3D hinges)
// ---------------------------------------------------------------------------

export class ThreeCutoutAnimator {
  readonly scene: THREE.Scene;
  readonly camera: THREE.PerspectiveCamera;
  readonly renderer: THREE.WebGLRenderer;
  readonly puppetGroup: THREE.Group;
  readonly viewCards: Map<string, THREE.Mesh> = new Map();
  readonly partPivots: Map<string, THREE.Group> = new Map();
  readonly partMeshes: Map<string, THREE.Mesh> = new Map();
  readonly textures: Map<string, THREE.Texture> = new Map();

  private canvas: HTMLCanvasElement;
  private currentYaw = 0;
  private cardHeight: number;
  private basePath: string;
  private isDisposed = false;

  constructor(canvas: HTMLCanvasElement, options: AnimatorOptions = {}) {
    this.canvas = canvas;
    this.basePath = options.basePath ?? "/exports/vault_slices/hopper";
    this.cardHeight = options.cardHeight ?? 1.62;

    const w = canvas.clientWidth || canvas.width || 640;
    const h = canvas.clientHeight || canvas.height || 480;

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(options.backgroundColor ?? 0x16181d);

    this.camera = new THREE.PerspectiveCamera(
      options.fov ?? 34,
      w / h,
      options.near ?? 0.1,
      options.far ?? 100
    );
    this.camera.position.set(0, 0.92, options.cameraZ ?? 3.35);
    this.camera.lookAt(0, 0.85, 0);

    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
    this.renderer.setSize(w, h, false);
    this.renderer.setPixelRatio(Math.min(typeof window !== "undefined" ? window.devicePixelRatio : 1, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;

    const hemi = new THREE.HemisphereLight(0xfffcf5, 0x7c7872, 0.95);
    hemi.position.set(0, 5, 0);
    this.scene.add(hemi);

    const key = new THREE.DirectionalLight(0xfffbf2, 1.05);
    key.position.set(2.8, 4.8, 3.8);
    key.castShadow = true;
    this.scene.add(key);

    const fill = new THREE.DirectionalLight(0xeef4ff, 0.55);
    fill.position.set(-3.4, 2.6, 2.2);
    this.scene.add(fill);

    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(18, 18),
      new THREE.ShadowMaterial({ opacity: 0.22, color: 0x000000 })
    );
    floor.rotation.x = -Math.PI / 2;
    floor.position.y = 0;
    floor.receiveShadow = true;
    this.scene.add(floor);

    this.puppetGroup = new THREE.Group();
    this.puppetGroup.name = "CutoutPuppetGroup";
    this.scene.add(this.puppetGroup);
  }

  async load(basePath?: string): Promise<void> {
    if (basePath) this.basePath = basePath.replace(/\/+$/, "");
    const viewImages: Record<string, HTMLImageElement> = {};

    for (const v of TURNAROUND_VIEW_SPECS) {
      for (const candidate of v.candidates) {
        try {
          const img = await loadImage(`${this.basePath}/${candidate}`);
          viewImages[v.view] = img;
          break;
        } catch {
          // try next candidate filename
        }
      }
    }

    this.buildTurntableCards(viewImages);
    if (viewImages.front) {
      this.buildLayeredPuppetFromFront(viewImages.front);
    }
  }

  async loadCutoutRigPlan(
    planOrUrl:
      | string
      | {
          character_id: string;
          canvas_size: [number, number];
          parts: Array<{
            part: string;
            file: string;
            parent: string;
            pivot: [number, number];
            z_index: number;
          }>;
        },
    baseUrl?: string
  ): Promise<void> {
    const plan =
      typeof planOrUrl === "string"
        ? await fetch(planOrUrl).then((r) => r.json())
        : planOrUrl;
    const resolvedBase = (
      baseUrl ??
      (typeof planOrUrl === "string"
        ? planOrUrl.replace(/\/[^/]+$/, "")
        : this.basePath)
    ).replace(/\/+$/, "");

    this.partPivots.forEach((p) => {
      if (p.parent) p.parent.remove(p);
    });
    this.partPivots.clear();
    this.partMeshes.clear();

    const [cw, ch] = plan.canvas_size || [787, 1345];
    const aspect = cw / Math.max(1, ch);
    const fullW = this.cardHeight * aspect;
    const fullH = this.cardHeight;

    const root = new THREE.Group();
    root.name = "Cutout_Root";
    this.puppetGroup.add(root);
    this.partPivots.set("Cutout_Root", root);

    const aliasMap: Record<string, string> = {
      ear_left: "ear_l",
      ear_right: "ear_r",
      arm_left: "arm_l",
      arm_right: "arm_r",
      leg_left: "leg_l",
      leg_right: "leg_r",
      foot_left: "foot_l",
      foot_right: "foot_r",
    };

    const worldPosByPart: Record<string, [number, number]> = {
      root: [0, 0],
    };
    for (const p of plan.parts) {
      const wx = (p.pivot[0] / cw - 0.5) * fullW;
      const wy = (1.0 - p.pivot[1] / ch) * fullH;
      worldPosByPart[p.part] = [wx, wy];
      const group = new THREE.Group();
      const shortId = aliasMap[p.part] || p.part;
      group.name = `Cutout_${shortId}`;
      this.partPivots.set(group.name, group);
      this.partPivots.set(`Cutout_${p.part}`, group);
    }

    for (const p of plan.parts) {
      const shortId = aliasMap[p.part] || p.part;
      const group = this.partPivots.get(`Cutout_${shortId}`)!;
      const parentGroup =
        p.parent === "root"
          ? root
          : this.partPivots.get(`Cutout_${aliasMap[p.parent] || p.parent}`) || root;
      const [wx, wy] = worldPosByPart[p.part];
      const [pwx, pwy] = worldPosByPart[p.parent] || [0, 0];
      group.position.set(wx - pwx, wy - pwy, (p.z_index - 40) * 0.001);
      parentGroup.add(group);

      try {
        const img = await loadImage(`${resolvedBase}/${p.file}`);
        const tex = new THREE.Texture(img as any);
        tex.colorSpace = THREE.SRGBColorSpace;
        tex.needsUpdate = true;
        this.textures.set(`part_${shortId}`, tex);

        const geo = new THREE.PlaneGeometry(fullW, fullH);
        const mat = new THREE.MeshStandardMaterial({
          map: tex,
          transparent: true,
          alphaTest: 0.02,
          roughness: 0.85,
          metalness: 0.0,
          side: THREE.DoubleSide,
        });
        const mesh = new THREE.Mesh(geo, mat);
        mesh.name = `Mesh_${shortId}`;
        mesh.position.set(-wx, -wy + fullH * 0.5, 0);
        mesh.castShadow = true;
        mesh.receiveShadow = true;
        group.add(mesh);
        this.partMeshes.set(shortId, mesh);
      } catch {
        // continue if a layer image fails to load
      }
    }
  }

  buildTurntableCards(viewImages: Record<string, HTMLImageElement | HTMLCanvasElement>): void {
    this.viewCards.forEach((card) => {
      this.puppetGroup.remove(card);
      card.geometry.dispose();
      (card.material as THREE.Material).dispose?.();
    });
    this.viewCards.clear();

    TURNAROUND_VIEW_SPECS.forEach((v) => {
      const img = viewImages[v.view];
      if (!img) return;
      const tex = new THREE.Texture(img as any);
      tex.colorSpace = THREE.SRGBColorSpace;
      tex.needsUpdate = true;
      this.textures.set(`card_${v.view}`, tex);

      const aspect = (img.width || 512) / (img.height || 512);
      const w = this.cardHeight * aspect;
      const geo = new THREE.PlaneGeometry(w, this.cardHeight);
      const mat = new THREE.MeshStandardMaterial({
        map: tex,
        transparent: true,
        alphaTest: 0.04,
        roughness: 0.88,
        metalness: 0.0,
        side: THREE.DoubleSide,
      });
      const card = new THREE.Mesh(geo, mat);
      card.name = `ViewCard_${v.view}`;
      card.position.set(0, this.cardHeight * 0.5, -0.04);
      card.castShadow = true;
      card.receiveShadow = true;
      card.visible = false; // hidden when front layered puppet is active at yaw ~ 0
      this.puppetGroup.add(card);
      this.viewCards.set(v.view, card);
    });
  }

  buildLayeredPuppetFromFront(frontImg: HTMLImageElement | HTMLCanvasElement): void {
    this.partPivots.forEach((p) => {
      if (p.parent) p.parent.remove(p);
    });
    this.partPivots.clear();
    this.partMeshes.clear();

    const aspect = (frontImg.width || 512) / (frontImg.height || 512);
    const fullW = this.cardHeight * aspect;
    const fullH = this.cardHeight;

    const root = new THREE.Group();
    root.name = "Cutout_Root";
    root.position.set(0, 0, 0);
    this.puppetGroup.add(root);
    this.partPivots.set("Cutout_Root", root);

    const makePartPlane = (
      spec: CutoutPartSpec,
      parentGroup: THREE.Group,
      parentPivotWorld: [number, number]
    ): THREE.Group => {
      const worldPivotX = (spec.pivotNorm[0] - 0.5) * fullW;
      const worldPivotY = (1.0 - spec.pivotNorm[1]) * fullH;

      const pivotGroup = new THREE.Group();
      pivotGroup.name = `Cutout_${spec.id}`;
      pivotGroup.position.set(
        worldPivotX - parentPivotWorld[0],
        worldPivotY - parentPivotWorld[1],
        spec.zOffset
      );
      parentGroup.add(pivotGroup);
      this.partPivots.set(pivotGroup.name, pivotGroup);

      const croppedCanvas = cropCanvasRegion(
        frontImg,
        spec.cropNorm[0],
        spec.cropNorm[1],
        spec.cropNorm[2],
        spec.cropNorm[3]
      );
      const tex = new THREE.CanvasTexture(croppedCanvas);
      tex.colorSpace = THREE.SRGBColorSpace;
      tex.needsUpdate = true;
      this.textures.set(`part_${spec.id}`, tex);

      const geo = new THREE.PlaneGeometry(fullW, fullH);
      const mat = new THREE.MeshStandardMaterial({
        map: tex,
        transparent: true,
        alphaTest: 0.04,
        roughness: 0.85,
        metalness: 0.0,
        side: THREE.DoubleSide,
      });
      const mesh = new THREE.Mesh(geo, mat);
      mesh.name = `Mesh_${spec.id}`;
      // Offset plane so the full-frame canvas aligns its pivot at pivotGroup (0,0,0)
      mesh.position.set(-worldPivotX, -worldPivotY + fullH * 0.5, 0);
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      pivotGroup.add(mesh);
      this.partMeshes.set(spec.id, mesh);
      return pivotGroup;
    };

    const byId = Object.fromEntries(DEFAULT_AUTO_PARTS.map((p) => [p.id, p]));
    const worldOf = (spec: CutoutPartSpec): [number, number] => [
      (spec.pivotNorm[0] - 0.5) * fullW,
      (1.0 - spec.pivotNorm[1]) * fullH,
    ];
    const torsoWorld = worldOf(byId.torso);
    const headWorld = worldOf(byId.head);
    const legLWorld = worldOf(byId.leg_l);
    const legRWorld = worldOf(byId.leg_r);

    const torsoGroup = makePartPlane(byId.torso, root, [0, 0]);
    const headGroup = makePartPlane(byId.head, torsoGroup, torsoWorld);
    makePartPlane(byId.ear_l, headGroup, headWorld);
    makePartPlane(byId.ear_r, headGroup, headWorld);
    makePartPlane(byId.arm_l, torsoGroup, torsoWorld);
    makePartPlane(byId.arm_r, torsoGroup, torsoWorld);
    const legLGroup = makePartPlane(byId.leg_l, root, [0, 0]);
    const legRGroup = makePartPlane(byId.leg_r, root, [0, 0]);
    makePartPlane(byId.foot_l, legLGroup, legLWorld);
    makePartPlane(byId.foot_r, legRGroup, legRWorld);
  }

  setYaw(degVal: number): void {
    this.currentYaw = degVal;
    const normDeg = ((((degVal + 180) % 360) + 360) % 360) - 180;

    let best = TURNAROUND_VIEW_SPECS[0];
    let bestDist = 999;
    for (const v of TURNAROUND_VIEW_SPECS) {
      const d = Math.abs((((normDeg - v.deg + 180) % 360) + 360) % 360 - 180);
      if (d < bestDist) {
        bestDist = d;
        best = v;
      }
    }

    const cutoutRoot = this.partPivots.get("Cutout_Root");
    const useLayeredFront = best.view === "front" && !!cutoutRoot;
    if (cutoutRoot) {
      cutoutRoot.visible = useLayeredFront;
      cutoutRoot.rotation.y = (normDeg * Math.PI) / 180 * 0.35;
    }

    this.viewCards.forEach((card, viewKey) => {
      card.visible = !useLayeredFront && viewKey === best.view;
    });
  }

  setPose(pose: PoseTransforms, sliders?: SliderValues): void {
    const root = this.partPivots.get("Cutout_Root");
    if (root) {
      root.position.y = pose.rootY;
      root.scale.set(pose.rootScale[0], pose.rootScale[1], pose.rootScale[2]);
    }
    const torso = this.partPivots.get("Cutout_torso");
    if (torso) {
      torso.rotation.z = pose.spine[2] * 1.4 - pose.spine[1] * 0.35;
    }
    const head = this.partPivots.get("Cutout_head");
    if (head) {
      head.rotation.z = pose.head[2] * 1.5 + pose.head[1] * 0.25;
    }
    const splay = sliders ? deg(sliders.earSplay - 14) * 0.35 : 0;
    const ears = this.partPivots.get("Cutout_ears");
    if (ears) {
      ears.rotation.z = pose.earL[2] * 0.6 + splay;
    }
    const earL = this.partPivots.get("Cutout_ear_l");
    const earR = this.partPivots.get("Cutout_ear_r");
    if (earL) earL.rotation.z = pose.earL[2] * 0.7 + splay;
    if (earR) earR.rotation.z = pose.earR[2] * 0.7 - splay;

    const armL = this.partPivots.get("Cutout_arm_l");
    const armR = this.partPivots.get("Cutout_arm_r");
    if (armL) armL.rotation.z = pose.armL[0] * 0.65 + (pose.armL[2] + 0.14) * 0.55;
    if (armR) armR.rotation.z = -pose.armR[0] * 0.65 + (pose.armR[2] - 0.14) * 0.55;

    const legL = this.partPivots.get("Cutout_leg_l");
    const legR = this.partPivots.get("Cutout_leg_r");
    if (legL) legL.rotation.z = pose.legL[0] * 0.55;
    if (legR) legR.rotation.z = -pose.legR[0] * 0.55;

    const footL = this.partPivots.get("Cutout_foot_l");
    const footR = this.partPivots.get("Cutout_foot_r");
    if (footL) footL.rotation.z = (pose.kneeL ?? 0) * 0.45 - pose.legL[0] * 0.25;
    if (footR) footR.rotation.z = -(pose.kneeR ?? 0) * 0.45 + pose.legR[0] * 0.25;
  }

  setBlink(blink: number): void {
    const headMesh = this.partMeshes.get("head");
    if (headMesh) {
      headMesh.scale.y = 1.0 - blink * 0.04;
    }
  }

  update(_dt: number): void {}

  render(): void {
    if (this.isDisposed) return;
    this.renderer.render(this.scene, this.camera);
  }

  resize(w: number, h: number): void {
    this.camera.aspect = w / Math.max(1, h);
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h, false);
  }

  dispose(): void {
    this.isDisposed = true;
    this.viewCards.forEach((c) => {
      c.geometry.dispose();
      (c.material as THREE.Material).dispose?.();
    });
    this.partMeshes.forEach((m) => {
      m.geometry.dispose();
      (m.material as THREE.Material).dispose?.();
    });
    this.textures.forEach((t) => t.dispose());
    this.renderer.dispose();
  }
}

function deg(d: number): number {
  return (d * Math.PI) / 180;
}

// ---------------------------------------------------------------------------
// p5.js / Canvas2D Pose-Sheet & Multi-Row Sprite-Cell Animator (Track A)
// Properly slices uneven multi-row sheets (e.g. 5 poses Row 1 + 6 poses Row 2)
// using detectRowBands + detectPoseCells + floodFillRemoveBg!
// ---------------------------------------------------------------------------

export interface P5PuppetOptions {
  poseSheet?: string;
  frameUrls?: string[];
  fps?: number;
  scale?: number;
  x?: number;
  y?: number;
}

export interface ExtractedPoseFrame {
  label: string;
  canvas: HTMLCanvasElement;
}

export class P5PuppetAnimator {
  readonly p5: any;
  public frames: ExtractedPoseFrame[] = [];
  private scale = 1;
  private x = 0;
  private y = 0;
  private mode: "idle" | "walk" | "run" | "jump" | "wave" | "talk" | "explore" = "walk";
  private energy = 1.0;
  private fps = 8;
  private elapsed = 0;
  private currentFrameIndex = 0;

  constructor(p5Instance: any, options: P5PuppetOptions = {}) {
    this.p5 = p5Instance;
    this.scale = options.scale ?? 1;
    this.x = options.x ?? 0;
    this.y = options.y ?? 0;
    this.fps = options.fps ?? 8;
  }

  // Slice any arbitrary single/multi-row pose sheet cleanly using border flood-fill
  async loadPoseSheet(urlOrImg: string | HTMLImageElement, tolerance = 28): Promise<number> {
    const img = typeof urlOrImg === "string" ? await loadImage(urlOrImg) : urlOrImg;
    const w = img.naturalWidth || img.width;
    const h = img.naturalHeight || img.height;
    const work = document.createElement("canvas");
    work.width = w;
    work.height = h;
    const ctx = work.getContext("2d")!;
    ctx.drawImage(img, 0, 0);
    const imgData = ctx.getImageData(0, 0, w, h);
    const rgba = imgData.data;

    const bg = estimateSheetBg(rgba, w, h);
    const bands = detectRowBands(rgba, w, h, bg, tolerance);
    const extracted: ExtractedPoseFrame[] = [];

    const ACTION_NAMES = [
      "standing", "walking", "running", "jumping", "dancing",
      "waving", "clapping", "looking_up", "looking_down", "arms_wide", "sitting"
    ];

    for (const band of bands) {
      const cells = detectPoseCells(rgba, w, h, band, bg, tolerance);
      for (const cell of cells) {
        // Trim any bottom text caption band inside the cell before flood-filling
        const tightBounds = floodFillRemoveBg(rgba, w, h, cell, bg, tolerance);
        if (tightBounds.x1 <= tightBounds.x0 + 8 || tightBounds.y1 <= tightBounds.y0 + 8) continue;
        const sliceCanvas = renderNormalizedSlice(rgba, w, h, tightBounds, 256, 256, false);
        const idx = extracted.length;
        extracted.push({
          label: ACTION_NAMES[idx] || `pose_${idx + 1}`,
          canvas: sliceCanvas,
        });
      }
    }

    if (extracted.length > 0) {
      this.frames = extracted;
      this.currentFrameIndex = 0;
    }
    return this.frames.length;
  }

  async loadFrameUrls(urls: string[], labels?: string[]): Promise<number> {
    const loaded: ExtractedPoseFrame[] = [];
    for (let i = 0; i < urls.length; i++) {
      try {
        const img = await loadImage(urls[i]);
        const c = document.createElement("canvas");
        c.width = 256;
        c.height = 256;
        const ctx = c.getContext("2d")!;
        const scale = Math.min(240 / (img.width || 256), 240 / (img.height || 256));
        const dw = (img.width || 256) * scale;
        const dh = (img.height || 256) * scale;
        ctx.drawImage(img, (256 - dw) * 0.5, 256 - 8 - dh, dw, dh);
        loaded.push({ label: labels?.[i] || `pose_${i + 1}`, canvas: c });
      } catch {
        // skip missing frame
      }
    }
    if (loaded.length > 0) {
      this.frames = loaded;
      this.currentFrameIndex = 0;
    }
    return this.frames.length;
  }

  setPoseCycle(
    mode: "idle" | "walk" | "run" | "jump" | "wave" | "talk" | "explore",
    energy = 1.0,
    fps = 8
  ): void {
    this.mode = mode;
    this.energy = energy;
    this.fps = fps;
  }

  private getSequenceIndices(): number[] {
    const n = this.frames.length;
    if (n === 0) return [0];
    if (n === 1) return [0];
    const clamp = (i: number) => Math.min(n - 1, Math.max(0, i));
    if (this.mode === "idle") return [clamp(0), clamp(9)];
    if (this.mode === "walk") return [clamp(0), clamp(1), clamp(2), clamp(1)];
    if (this.mode === "run") return [clamp(1), clamp(2), clamp(3), clamp(2)];
    if (this.mode === "jump") return [clamp(0), clamp(3), clamp(4), clamp(3)];
    if (this.mode === "wave") return [clamp(0), clamp(5), clamp(6), clamp(5)];
    if (this.mode === "explore") return [clamp(0), clamp(7), clamp(8), clamp(9)];
    return Array.from({ length: n }, (_, i) => i);
  }

  advance(dt: number): number {
    if (this.frames.length === 0) return 0;
    this.elapsed += dt;
    const seq = this.getSequenceIndices();
    const step = Math.floor(this.elapsed * this.fps) % seq.length;
    this.currentFrameIndex = seq[step];
    return this.currentFrameIndex;
  }

  drawToCanvas2D(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    ctx.clearRect(0, 0, w, h);
    // Warm studio gradient floor
    const grad = ctx.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, "#1c1f26");
    grad.addColorStop(1, "#12141a");
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, w, h);

    if (this.frames.length === 0) return;
    const frameObj = this.frames[this.currentFrameIndex] || this.frames[0];

    const bounce =
      this.mode === "jump"
        ? Math.abs(Math.sin(this.elapsed * 5.2)) * 22 * this.energy
        : this.mode === "walk" || this.mode === "run"
        ? Math.abs(Math.sin(this.elapsed * 8.0)) * 8 * this.energy
        : Math.sin(this.elapsed * 2.4) * 3 * this.energy;

    const tilt =
      this.mode === "wave" || this.mode === "walk"
        ? Math.sin(this.elapsed * 4.5) * 0.04 * this.energy
        : 0;

    const squashY = 1.0 + Math.sin(this.elapsed * 7.0) * 0.03 * this.energy;
    const squashX = 1.0 - Math.sin(this.elapsed * 7.0) * 0.02 * this.energy;

    // Ground shadow
    ctx.save();
    ctx.fillStyle = "rgba(0,0,0,0.32)";
    ctx.beginPath();
    ctx.ellipse(w * 0.5, h - 22, 46 * (1 - bounce / 90), 10, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();

    const drawSize = Math.min(w, h) * 0.78 * this.scale;
    ctx.save();
    ctx.translate(w * 0.5, h - 18 - bounce);
    ctx.rotate(tilt);
    ctx.scale(squashX, squashY);
    ctx.drawImage(frameObj.canvas, -drawSize * 0.5, -drawSize, drawSize, drawSize);
    ctx.restore();

    // HUD label
    ctx.save();
    ctx.fillStyle = "rgba(18, 20, 26, 0.82)";
    ctx.fillRect(8, 8, 195, 22);
    ctx.fillStyle = "#5ce1e6";
    ctx.font = "600 10px monospace";
    ctx.fillText(
      `p5 2D CYCLE: ${this.mode.toUpperCase()} · ${frameObj.label} (${this.currentFrameIndex + 1}/${this.frames.length})`,
      14,
      22
    );
    ctx.restore();
  }

  draw(p5Instance?: any): void {
    const pp = p5Instance ?? this.p5;
    if (!pp) return;
    if (pp.drawingContext && pp.width && pp.height) {
      this.drawToCanvas2D(pp.drawingContext as CanvasRenderingContext2D, pp.width, pp.height);
    }
  }

  tick(dt: number, p5Instance?: any): void {
    this.advance(dt);
    this.draw(p5Instance);
  }

  dispose(): void {
    this.frames = [];
  }
}

export function createThreeCutoutAnimator(
  canvas: HTMLCanvasElement,
  options?: AnimatorOptions
): ThreeCutoutAnimator {
  return new ThreeCutoutAnimator(canvas, options);
}

export function createP5PuppetAnimator(
  p5: any,
  options?: P5PuppetOptions
): P5PuppetAnimator {
  return new P5PuppetAnimator(p5, options);
}

export function createP5PuppetSketch(
  elementOrCanvas: HTMLElement | HTMLCanvasElement | string,
  options: P5PuppetOptions & { width?: number; height?: number } = {}
): { animator: P5PuppetAnimator; stop: () => void } {
  const target =
    typeof elementOrCanvas === "string"
      ? (document.querySelector(elementOrCanvas) as HTMLElement)
      : elementOrCanvas;
  const w = options.width ?? 320;
  const h = options.height ?? 320;
  let canvas: HTMLCanvasElement;
  if (target instanceof HTMLCanvasElement) {
    canvas = target;
  } else {
    canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    if (target) {
      target.innerHTML = "";
      target.appendChild(canvas);
    }
  }
  const ctx = canvas.getContext("2d")!;
  const p5Like = { width: canvas.width, height: canvas.height, drawingContext: ctx };
  const animator = new P5PuppetAnimator(p5Like, options);
  if (options.poseSheet) {
    void animator.loadPoseSheet(options.poseSheet);
  } else if (options.frameUrls?.length) {
    void animator.loadFrameUrls(options.frameUrls);
  }
  let rafId = 0;
  let last = performance.now() / 1000;
  const loop = () => {
    const now = performance.now() / 1000;
    const dt = Math.min(0.1, now - last);
    last = now;
    p5Like.width = canvas.width;
    p5Like.height = canvas.height;
    animator.tick(dt, p5Like);
    rafId = requestAnimationFrame(loop);
  };
  rafId = requestAnimationFrame(loop);
  return {
    animator,
    stop: () => cancelAnimationFrame(rafId),
  };
}
