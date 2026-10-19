// src/rig/rigBuilder.ts
// Modular rig builder extracted from buildBunnyExplorerCharacter.
// Each body region is a standalone function that attaches sub-meshes to the rig
// pivots and registers RigParts into RigState. No Blender required.

import * as THREE from "three";
import type { RigState, RigPivots, RigPart, Species } from "./rigState";

const MATS = new Map<string, any>();

function getOrCreateMat(name: string, hex: number): any {
  if (MATS.has(name)) return MATS.get(name);
  const feltTex = createFeltTexture(`#${hex.toString(16).padStart(6, "0")}`, 22, 3);
  const mat = new THREE.MeshStandardMaterial({
    name,
    color: hex,
    map: feltTex,
    bumpMap: feltTex,
    bumpScale: 0.015,
    roughness: 0.9,
    metalness: 0,
  });
  MATS.set(name, mat);
  return mat;
}

function createFeltTexture(baseHex: string, noiseAmount = 22, scale = 1): any {
  const texCanvas = document.createElement("canvas");
  texCanvas.width = 256;
  texCanvas.height = 256;
  const ctx = texCanvas.getContext("2d")!;
  const baseColor = new THREE.Color(baseHex);
  const rBase = Math.floor(baseColor.r * 255);
  const gBase = Math.floor(baseColor.g * 255);
  const bBase = Math.floor(baseColor.b * 255);
  const imgData = ctx.createImageData(256, 256);
  const data = imgData.data;
  let seed = 1337;
  function rand() {
    seed = (seed * 16807) % 2147483647;
    return (seed - 1) / 2147483646;
  }
  for (let i = 0; i < data.length; i += 4) {
    const n = (rand() - 0.5) * noiseAmount * 2;
    data[i] = Math.min(255, Math.max(0, rBase + n));
    data[i + 1] = Math.min(255, Math.max(0, gBase + n));
    data[i + 2] = Math.min(255, Math.max(0, bBase + n));
    data[i + 3] = 255;
  }
  ctx.putImageData(imgData, 0, 0);
  ctx.lineWidth = 1.1;
  for (let f = 0; f < 800; f++) {
    const x = rand() * 256;
    const y = rand() * 256;
    const len = 4 + rand() * 8;
    const ang = rand() * Math.PI * 2;
    const bright = rand() > 0.5 ? 14 : -14;
    ctx.strokeStyle = `rgba(${Math.min(255, Math.max(0, rBase + bright))},${Math.min(255, Math.max(0, gBase + bright))},${Math.min(255, Math.max(0, bBase + bright))},0.35)`;
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.quadraticCurveTo(x + Math.cos(ang + 0.5) * (len * 0.5), y + Math.sin(ang + 0.5) * (len * 0.5), x + Math.cos(ang) * len, y + Math.sin(ang) * len);
    ctx.stroke();
  }
  const tex = new THREE.CanvasTexture(texCanvas);
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.repeat.set(scale, scale);
  return tex;
}

function createRoundedBoxGeometry(w: number, h: number, d: number, radius = 0.02, smoothness = 4): any {
  const shape = new THREE.Shape();
  const eps = 0.00001;
  const r = Math.min(radius, w * 0.45, h * 0.45, d * 0.45) - eps;
  shape.absarc(eps, eps, eps, -Math.PI / 2, -Math.PI, true);
  shape.absarc(eps, h - r * 2, eps, Math.PI, Math.PI / 2, true);
  shape.absarc(w - r * 2, h - r * 2, eps, Math.PI / 2, 0, true);
  shape.absarc(w - r * 2, eps, eps, 0, -Math.PI / 2, true);
  const geom = new THREE.ExtrudeGeometry(shape, {
    depth: Math.max(0.001, d - radius * 2),
    bevelEnabled: true,
    bevelSegments: smoothness * 2,
    steps: 1,
    bevelSize: r,
    bevelThickness: radius,
    curveSegments: smoothness,
  });
  geom.center();
  geom.computeVertexNormals();
  return geom;
}

function createSpoonEarGeometry(width = 0.145, height = 0.38, depth = 0.044, bevel = 0.018): any {
  const shape = new THREE.Shape();
  const hw = width * 0.5;
  const baseW = hw * 0.62;
  const topW = hw * 1.05;
  shape.moveTo(-baseW, 0);
  shape.bezierCurveTo(-topW * 1.12, height * 0.35, -topW * 1.08, height * 0.78, -topW * 0.55, height * 0.94);
  shape.bezierCurveTo(-topW * 0.22, height * 1.02, topW * 0.22, height * 1.02, topW * 0.55, height * 0.94);
  shape.bezierCurveTo(topW * 1.08, height * 0.78, topW * 1.12, height * 0.35, baseW, 0);
  shape.quadraticCurveTo(0, -0.018, -baseW, 0);
  const geom = new THREE.ExtrudeGeometry(shape, {
    depth: Math.max(0.002, depth - bevel * 2),
    bevelEnabled: true,
    bevelSegments: 5,
    steps: 1,
    bevelSize: bevel,
    bevelThickness: bevel,
    curveSegments: 18,
  });
  geom.translate(0, 0, -depth * 0.25);
  geom.computeVertexNormals();
  return geom;
}

function tagCategory(root: any, categoryKey: string): void {
  root.userData = root.userData || {};
  root.userData.category = categoryKey;
  root.traverse((child: any) => {
    child.userData = child.userData || {};
    child.userData.category = categoryKey;
    if (child.isMesh) {
      child.castShadow = true;
      child.receiveShadow = true;
    }
  });
}

// ---------- sub-builders ----------

export function buildHeadAssembly(parent: any, pivots: RigPivots, parts: RigPart[]): void {
  const headPivot = new THREE.Group();
  headPivot.name = "Head_Pivot";
  headPivot.position.set(0, 0.96, 0);
  parent.add(headPivot);
  pivots.headPivot = headPivot;

  const headGroup = new THREE.Group();
  headGroup.name = "Head_Assembly";
  headPivot.add(headGroup);
  tagCategory(headGroup, "head");

  const craniumGeo = new THREE.SphereGeometry(0.275, 36, 28);
  craniumGeo.scale(1.06, 1.04, 0.98);
  const cranium = new THREE.Mesh(craniumGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
  cranium.name = "Head_Cranium_Mesh";
  cranium.position.set(0, 0.24, 0);
  headGroup.add(cranium);

  const lowerCheekGeo = new THREE.SphereGeometry(0.28, 36, 24);
  lowerCheekGeo.scale(1.18, 0.74, 1.02);
  const cheek = new THREE.Mesh(lowerCheekGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
  cheek.name = "Head_CheekMass_Mesh";
  cheek.position.set(0, 0.145, 0.03);
  headGroup.add(cheek);

  parts.push(
    { id: "P01", name: "Head_Cranium_Mesh", zone: "Head & Snout", bone: "DEF_Head", explode: [0, 0.28, 0], desc: "Upper plush cranium dome & forehead locks", node: cranium, visible: true, verified: true },
    { id: "P02", name: "Head_CheekMass_Mesh", zone: "Head & Snout", bone: "DEF_Head", explode: [0, 0.14, 0.18], desc: "Wide lower pear-shaped plush cheek volume", node: cheek, visible: true, verified: true },
  );

  // Muzzle pads + cheeks
  [-1, 1].forEach((side) => {
    const suffix = side < 0 ? "R" : "L";
    const padGeo = new THREE.SphereGeometry(0.115, 24, 20);
    padGeo.scale(1.18, 0.85, 1.08);
    const pad = new THREE.Mesh(padGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    pad.name = `Snout_MuzzlePad_${suffix}`;
    pad.position.set(side * 0.078, 0.145, 0.205);
    headGroup.add(pad);

    const blushGeo = new THREE.SphereGeometry(0.068, 20, 16);
    blushGeo.scale(1.1, 0.78, 0.35);
    const blush = new THREE.Mesh(blushGeo, getOrCreateMat("MAT_Rosy_Cheek_Felt", 0xeba496));
    blush.name = `Cheek_Blush_${suffix}`;
    blush.position.set(side * 0.228, 0.148, 0.195);
    blush.rotation.y = side * 0.62;
    headGroup.add(blush);

    parts.push(
      { id: side < 0 ? "P03" : "P04", name: pad.name, zone: "Head & Snout", bone: "DEF_Head", explode: [side * 0.16, 0.12, 0.28], desc: `${suffix} chubby muzzle whisker pad`, node: pad, visible: true, verified: true },
    );
  });

  const chinGeo = new THREE.SphereGeometry(0.12, 24, 18);
  chinGeo.scale(1.18, 0.58, 0.95);
  const chin = new THREE.Mesh(chinGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
  chin.name = "Head_Chin_Mesh";
  chin.position.set(0, 0.042, 0.185);
  headGroup.add(chin);

  const tuftGroup = new THREE.Group();
  tuftGroup.name = "Head_Top_Tufts";
  tuftGroup.position.set(0, 0.515, 0.035);
  [{ x: 0, z: 0.015, rotZ: 0, rotX: 0.22, s: 1.05 }, { x: -0.028, z: 0.005, rotZ: 0.34, rotX: 0.12, s: 0.92 }, { x: 0.028, z: 0.005, rotZ: -0.34, rotX: 0.12, s: 0.92 }, { x: 0.012, z: -0.01, rotZ: -0.15, rotX: 0.05, s: 0.80 }].forEach((t, idx) => {
    const tuftGeo = new THREE.SphereGeometry(0.024 * t.s, 14, 12);
    tuftGeo.scale(0.85, 2.2, 0.85);
    const tuft = new THREE.Mesh(tuftGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    tuft.name = `Head_FurTuft_${idx + 1}`;
    tuft.position.set(t.x, 0.02, t.z);
    tuft.rotation.set(t.rotX, 0, t.rotZ);
    tuftGroup.add(tuft);
  });
  headGroup.add(tuftGroup);

  parts.push({ id: "P05", name: "Head_Top_Tufts", zone: "Head & Snout", bone: "DEF_Head", explode: [0, 0.42, 0.08], desc: "Top sculpted crown fur tuft forelocks", node: tuftGroup, visible: true, verified: true });
}

export function buildFacialFeatures(headPivot: any, pivots: RigPivots, parts: RigPart[]): void {
  const faceGroup = new THREE.Group();
  faceGroup.name = "Facial_Features_Group";
  headPivot.add(faceGroup);
  tagCategory(faceGroup, "face");
  pivots.smileGroup = new THREE.Group();
  pivots.smileGroup.name = "Face_Smile_Group";
  pivots.smileGroup.position.set(0, 0.105, 0.258);
  faceGroup.add(pivots.smileGroup);

  const noseGeo = new THREE.SphereGeometry(0.038, 20, 16);
  noseGeo.scale(1.28, 0.85, 0.95);
  const nose = new THREE.Mesh(noseGeo, getOrCreateMat("MAT_Pink_Nose_Felt", 0xdf887d));
  nose.name = "Face_Pink_Nose";
  nose.position.set(0, 0.195, 0.312);
  faceGroup.add(nose);

  pivots.eyelidUpperMeshes = [];
  pivots.eyelidLowerMeshes = [];
  pivots.eyeGazePivots = [];
  pivots.eyebrowMeshes = [];

  [-1, 1].forEach((side) => {
    const suffix = side < 0 ? "R" : "L";
    const eyeSocket = new THREE.Group();
    eyeSocket.name = `Eye_Unit_${suffix}`;
    eyeSocket.position.set(side * 0.118, 0.252, 0.255);
    eyeSocket.rotation.y = side * 0.24;

    const scleraGeo = new THREE.SphereGeometry(0.045, 22, 18);
    scleraGeo.scale(1.04, 1.08, 0.58);
    const sclera = new THREE.Mesh(scleraGeo, getOrCreateMat("MAT_Eye_Sclera_White", 0xfaf6f0));
    sclera.name = `Eye_Sclera_${suffix}`;
    sclera.position.set(-side * 0.004, 0, -0.003);
    eyeSocket.add(sclera);

    const gazePivot = new THREE.Group();
    gazePivot.name = `Eye_Gaze_Pivot_${suffix}`;
    eyeSocket.add(gazePivot);
    pivots.eyeGazePivots.push(gazePivot);

    const irisGeo = new THREE.SphereGeometry(0.040, 22, 18);
    irisGeo.scale(0.96, 1.05, 0.64);
    const iris = new THREE.Mesh(irisGeo, getOrCreateMat("MAT_Eye_Iris_DarkBrown", 0x24160f));
    iris.name = `Eye_Iris_${suffix}`;
    iris.position.set(side * 0.003, 0.002, 0.006);
    gazePivot.add(iris);

    const upperLidGeo = new THREE.SphereGeometry(0.048, 20, 14, 0, Math.PI * 2, 0, Math.PI * 0.52);
    upperLidGeo.scale(1.06, 1.08, 0.68);
    const upperLid = new THREE.Mesh(upperLidGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    upperLid.name = `Eyelid_Upper_${suffix}`;
    upperLid.rotation.x = -1.15;
    eyeSocket.add(upperLid);
    pivots.eyelidUpperMeshes.push(upperLid);

    const lowerLidGeo = new THREE.SphereGeometry(0.047, 20, 14, 0, Math.PI * 2, Math.PI * 0.5, Math.PI * 0.5);
    lowerLidGeo.scale(1.05, 1.05, 0.66);
    const lowerLid = new THREE.Mesh(lowerLidGeo, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    lowerLid.name = `Eyelid_Lower_${suffix}`;
    lowerLid.rotation.x = 0.85;
    eyeSocket.add(lowerLid);
    pivots.eyelidLowerMeshes.push(lowerLid);

    faceGroup.add(eyeSocket);

    const browGeo = new THREE.TorusGeometry(0.048, 0.0095, 10, 20, Math.PI * 0.62);
    const brow = new THREE.Mesh(browGeo, getOrCreateMat("MAT_Eyebrow_Brown_Felt", 0x6e4d35));
    brow.name = `Face_Eyebrow_${suffix}`;
    brow.position.set(side * 0.122, 0.325, 0.242);
    brow.rotation.set(-0.18, side * 0.28, Math.PI * 0.18 - side * 0.18);
    faceGroup.add(brow);
    pivots.eyebrowMeshes.push(brow);

    parts.push(
      { id: side < 0 ? "P08" : "P09", name: `Eye_Unit_${suffix}`, zone: "Face & Mouth", bone: `DEF_Eye.${suffix}`, explode: [side * 0.18, 0.18, 0.35], desc: `${suffix} eye, gaze pivot, & upper/lower blink eyelids`, node: eyeSocket, visible: true, verified: true },
    );
  });

  parts.push({ id: "P10", name: "Face_Pink_Nose", zone: "Face & Mouth", bone: "DEF_Head", explode: [0, 0.14, 0.42], desc: "Triangle sculpted felt nose button", node: nose, visible: true, verified: true });
  parts.push({ id: "P11", name: "Face_Smile_Group", zone: "Face & Mouth", bone: "DEF_Jaw", explode: [0, 0.02, 0.38], desc: "Articulated jaw viseme cavity, pink tongue & buck teeth", node: pivots.smileGroup, visible: true, verified: true });
}

export function buildSpoonEars(headPivot: any, pivots: RigPivots, parts: RigPart[]): void {
  const sharedOuter = createSpoonEarGeometry(0.152, 0.385, 0.046, 0.018);
  const sharedInner = createSpoonEarGeometry(0.102, 0.325, 0.024, 0.010);
  const sharedRim = createSpoonEarGeometry(0.146, 0.375, 0.021); // reused as rim proxy

  [-1, 1].forEach((side) => {
    const suffix = side < 0 ? "R" : "L";
    const earPivot = new THREE.Group();
    earPivot.name = `Ear_Pivot_${suffix}`;
    earPivot.position.set(side * 0.105, 0.46, -0.01);
    earPivot.rotation.z = -side * deg(14);
    earPivot.rotation.x = deg(-4);
    earPivot.rotation.y = side * deg(12);

    const outer = new THREE.Mesh(sharedOuter, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    outer.name = `Ear_Outer_Felt_${suffix}`;
    outer.position.set(0, 0.02, 0);
    earPivot.add(outer);

    const inner = new THREE.Mesh(sharedInner, getOrCreateMat("MAT_Pink_Inner_Ear", 0xe39787));
    inner.name = `Ear_Inner_Pink_${suffix}`;
    inner.position.set(0, 0.042, 0.018);
    earPivot.add(inner);

    const rim = new THREE.Mesh(sharedRim, getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    rim.name = `Ear_Rim_Fold_${suffix}`;
    rim.position.set(0, 0.02, 0.008);
    earPivot.add(rim);

    const tipPivot = new THREE.Group();
    tipPivot.name = `Ear_Tip_Pivot_${suffix}`;
    tipPivot.position.set(0, 0.24, 0);
    earPivot.add(tipPivot);

    if (side > 0) pivots.earLPivot = earPivot; else pivots.earRPivot = earPivot;
    if (side > 0) pivots.earTipLPivot = tipPivot; else pivots.earTipRPivot = tipPivot;

    headPivot.add(earPivot);
    tagCategory(earPivot, "ears");

    parts.push({
      id: side < 0 ? "P07" : "P06",
      name: `Ear_Pivot_${suffix}`,
      zone: "Ears (L/R)",
      bone: `DEF_Ear.${suffix}`,
      explode: [side * 0.28, 0.38, 0],
      desc: `${suffix} rounded spoon ear shell + continuous plush rim`,
      node: earPivot,
      visible: true,
      verified: true,
    });
  });
}

function deg(d: number): number { return (d * Math.PI) / 180; }

export function buildTorsoAndVest(parent: any, pivots: RigPivots, parts: RigPart[]): void {
  const torsoGroup = new THREE.Group();
  torsoGroup.name = "Torso_And_Shirt_Group";
  parent.add(torsoGroup);
  pivots.torsoGroup = torsoGroup;
  tagCategory(torsoGroup, "shirt");

  const shirtGeo = new THREE.CylinderGeometry(0.175, 0.205, 0.42, 28);
  shirtGeo.scale(1.08, 1.0, 0.88);
  const shirt = new THREE.Mesh(shirtGeo, getOrCreateMat("MAT_Cream_Undershirt", 0xefe6d5));
  shirt.name = "Shirt_Cream_Torso";
  shirt.position.set(0, 0.77, 0);
  torsoGroup.add(shirt);

  const vestGroup = new THREE.Group();
  vestGroup.name = "Outfit_Orange_Vest_Group";
  parent.add(vestGroup);
  pivots.vestGroup = vestGroup;
  tagCategory(vestGroup, "vest");

  const vestBack = new THREE.Mesh(new THREE.CylinderGeometry(0.185, 0.215, 0.40, 24, 1, true, Math.PI * 0.55, Math.PI * 0.9), getOrCreateMat("MAT_Orange_Felt_Vest", 0xcc5b16));
  vestBack.scale(1.12, 1.0, 0.92);
  vestBack.name = "Vest_Back_Panel";
  vestBack.position.set(0, 0.775, -0.005);
  vestGroup.add(vestBack);

  [-1, 1].forEach((side) => {
    const suffix = side < 0 ? "R" : "L";
    const panel = new THREE.Mesh(createRoundedBoxGeometry(0.125, 0.39, 0.195, 0.025, 4), getOrCreateMat("MAT_Orange_Felt_Vest", 0xcc5b16));
    panel.name = `Vest_Front_Panel_${suffix}`;
    panel.position.set(side * 0.122, 0.77, 0.055);
    panel.rotation.y = -side * 0.18;
    panel.rotation.z = side * 0.04;
    vestGroup.add(panel);
    parts.push({
      id: side < 0 ? "P14" : "P13",
      name: panel.name,
      zone: "Upper Torso",
      bone: "DEF_Chest",
      explode: [side * 0.26, 0.05, 0.22],
      desc: `${suffix} front orange felt vest lapel`,
      node: panel,
      visible: true,
      verified: true,
    });
  });

  parts.push({ id: "P12", name: "Shirt_Cream_Torso", zone: "Upper Torso", bone: "DEF_Chest", explode: [0, 0.05, 0], desc: "Core upper torso plush body / cream undershirt", node: shirt, visible: true, verified: true });
  parts.push({ id: "P15", name: "Vest_Back_Panel", zone: "Upper Torso", bone: "DEF_Chest", explode: [0, 0.05, -0.22], desc: "Rear tailored vest back panel", node: vestBack, visible: true, verified: true });
}

export function buildArmsAndPaws(parent: any, pivots: RigPivots, parts: RigPart[]): void {
  [-1, 1].forEach((side) => {
    const suffix = side < 0 ? "R" : "L";
    const armPivot = new THREE.Group();
    armPivot.name = `Arm_Pivot_${suffix}`;
    armPivot.position.set(side * 0.215, 0.91, 0);
    armPivot.rotation.z = -side * deg(8);

    const sleeve = new THREE.Mesh(new THREE.CylinderGeometry(0.068, 0.078, 0.155, 20), getOrCreateMat("MAT_Cream_Undershirt", 0xefe6d5));
    sleeve.name = `Shirt_Sleeve_${suffix}`;
    sleeve.position.set(side * 0.025, -0.065, 0);
    sleeve.rotation.z = -side * 0.18;
    armPivot.add(sleeve);

    const elbowPivot = new THREE.Group();
    elbowPivot.name = `Elbow_Pivot_${suffix}`;
    elbowPivot.position.set(side * 0.038, -0.13, 0);
    elbowPivot.rotation.x = -deg(12);
    armPivot.add(elbowPivot);

    const forearm = new THREE.Mesh(new THREE.CylinderGeometry(0.056, 0.052, 0.175, 20), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    forearm.name = `Arm_Forearm_${suffix}`;
    forearm.position.set(side * 0.008, -0.085, 0.008);
    forearm.rotation.z = -side * 0.06;
    elbowPivot.add(forearm);

    const wristPivot = new THREE.Group();
    wristPivot.name = `Wrist_Pivot_${suffix}`;
    wristPivot.position.set(side * 0.014, -0.175, 0.015);
    elbowPivot.add(wristPivot);

    const paw = new THREE.Mesh(new THREE.SphereGeometry(0.058, 20, 18), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    paw.scale(0.95, 1.05, 0.82);
    paw.name = `Arm_Paw_Hand_${suffix}`;
    paw.position.set(0, -0.022, 0.005);
    wristPivot.add(paw);

    const pad = new THREE.Mesh(new THREE.SphereGeometry(0.034, 16, 12), getOrCreateMat("MAT_Pink_Inner_Ear", 0xe39787));
    pad.scale(1.0, 1.0, 0.45);
    pad.name = `Arm_Paw_Pad_${suffix}`;
    pad.position.set(-side * 0.012, -0.024, 0.038);
    wristPivot.add(pad);

    const thumb = new THREE.Mesh(new THREE.SphereGeometry(0.024, 14, 12), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    thumb.scale(0.88, 1.45, 0.88);
    thumb.name = `Arm_Paw_Thumb_${suffix}`;
    thumb.position.set(-side * 0.042, -0.018, 0.034);
    thumb.rotation.set(0.38, 0, side * 0.48);
    wristPivot.add(thumb);

    [-0.030, -0.010, 0.010, 0.030].forEach((fX, fIdx) => {
      const finger = new THREE.Mesh(new THREE.SphereGeometry(0.019, 14, 12), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
      const len = (fIdx === 1 || fIdx === 2) ? 1.48 : 1.32;
      finger.scale(0.86, len, 0.90);
      finger.name = `Arm_Paw_Finger_${fIdx + 1}_${suffix}`;
      finger.position.set(fX, -0.068, 0.016 + (fIdx === 1 || fIdx === 2 ? 0.005 : 0));
      finger.rotation.x = 0.28;
      wristPivot.add(finger);
    });

    if (side > 0) { pivots.armLPivot = armPivot; pivots.elbowLPivot = elbowPivot; pivots.wristLPivot = wristPivot; }
    else { pivots.armRPivot = armPivot; pivots.elbowRPivot = elbowPivot; pivots.wristRPivot = wristPivot; }

    parent.add(armPivot);
    tagCategory(armPivot, "limbs");

    parts.push({
      id: side < 0 ? "P20" : "P19",
      name: `Arm_Pivot_${suffix}`,
      zone: "Upper Limbs",
      bone: `DEF_UpperArm.${suffix}`,
      explode: [side * 0.34, 0.08, 0],
      desc: `${suffix} shoulder & cream short sleeve`,
      node: armPivot,
      visible: true,
      verified: true,
    });
    parts.push({
      id: side < 0 ? "P22" : "P21",
      name: `Wrist_Pivot_${suffix}`,
      zone: "Hands & Paws",
      bone: `DEF_Hand.${suffix}`,
      explode: [side * 0.46, -0.04, 0.18],
      desc: `${suffix} articulated wrist, palm pad, thumb & 4 paw fingers`,
      node: wristPivot,
      visible: true,
      verified: true,
    });
  });
}

export function buildBackpack(parent: any, pivots: RigPivots, parts: RigPart[]): void {
  const packGroup = new THREE.Group();
  packGroup.name = "Explorer_Backpack_Group";
  parent.add(packGroup);
  pivots.backpackGroup = packGroup;
  tagCategory(packGroup, "backpack");

  const body = new THREE.Mesh(createRoundedBoxGeometry(0.32, 0.36, 0.17, 0.042, 4), getOrCreateMat("MAT_Backpack_Olive_Felt", 0x6c7131));
  body.name = "Backpack_Main_Olive_Body";
  body.position.set(0, 0.78, -0.195);
  packGroup.add(body);

  const flap = new THREE.Mesh(createRoundedBoxGeometry(0.31, 0.145, 0.175, 0.035, 4), getOrCreateMat("MAT_Backpack_Olive_Felt", 0x6c7131));
  flap.name = "Backpack_Top_Olive_Flap";
  flap.position.set(0, 0.905, -0.202);
  flap.rotation.x = 0.08;
  packGroup.add(flap);

  const pouch = new THREE.Mesh(createRoundedBoxGeometry(0.235, 0.155, 0.075, 0.022, 4), getOrCreateMat("MAT_Backpack_Brown_Pouch", 0x5c3b1e));
  pouch.name = "Backpack_Rear_Brown_Pouch";
  pouch.position.set(0, 0.705, -0.295);
  packGroup.add(pouch);

  parts.push({ id: "P16", name: "Backpack_Main_Olive_Body", zone: "Gear & Pack", bone: "DEF_Chest", explode: [0, 0.12, -0.38], desc: "Main olive pack body & shoulder straps", node: body, visible: true, verified: true });
  parts.push({ id: "P17", name: "Backpack_Top_Olive_Flap", zone: "Gear & Pack", bone: "DEF_Chest", explode: [0, 0.28, -0.42], desc: "Top pack flap, brown strap & carry loop", node: flap, visible: true, verified: true });
  parts.push({ id: "P18", name: "Backpack_Rear_Brown_Pouch", zone: "Gear & Pack", bone: "DEF_Chest", explode: [-0.28, 0, -0.48], desc: "Rear/side utility pouch & brass clasp hardware", node: pouch, visible: true, verified: true });
}

export function buildPelvisAndShorts(parent: any, pivots: RigPivots, parts: RigPart[]): void {
  const pelvisPivot = new THREE.Group();
  pelvisPivot.name = "Pelvis_Pivot";
  pelvisPivot.position.set(0, 0.52, 0);
  parent.add(pelvisPivot);
  pivots.pelvisPivot = pelvisPivot;

  const pelvisOffset = new THREE.Group();
  pelvisOffset.position.set(0, -0.52, 0);
  pelvisPivot.add(pelvisOffset);

  const shortsGroup = new THREE.Group();
  shortsGroup.name = "Outfit_Denim_Shorts_Group";
  pelvisOffset.add(shortsGroup);
  pivots.shortsGroup = shortsGroup;
  tagCategory(shortsGroup, "shorts");

  const pelvisGeo = new THREE.CylinderGeometry(0.198, 0.212, 0.18, 28);
  pelvisGeo.scale(1.08, 1.0, 0.9);
  const pelvis = new THREE.Mesh(pelvisGeo, getOrCreateMat("MAT_Denim_Blue_Shorts", 0x264b73));
  pelvis.name = "Shorts_Pelvis_Waistband";
  pelvis.position.set(0, 0.525, 0);
  shortsGroup.add(pelvis);

  const tailPivot = new THREE.Group();
  tailPivot.name = "Bunny_Tail_Pivot";
  tailPivot.position.set(0, 0.555, -0.195);
  const tail = new THREE.Mesh(new THREE.SphereGeometry(0.058, 20, 16), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
  tail.name = "Bunny_Cotton_Tail";
  tailPivot.add(tail);
  pivots.tailPivot = tailPivot;
  pivots.tailMesh = tail;
  pelvisOffset.add(tailPivot);
  tagCategory(tailPivot, "limbs");

  parts.push({ id: "P23", name: "Shorts_Pelvis_Waistband", zone: "Pelvis & Tail", bone: "DEF_Pelvis", explode: [0, -0.08, 0.16], desc: "Lower pelvis denim waistband & rear pockets", node: pelvis, visible: true, verified: true });
  parts.push({ id: "P24", name: "Bunny_Tail_Pivot", zone: "Pelvis & Tail", bone: "DEF_Tail", explode: [0, -0.06, -0.38], desc: "Articulated rear cotton tail / striped cat tail", node: tailPivot, visible: true, verified: true });
}

export function buildLegsAndFeet(parent: any, pivots: RigPivots, parts: RigPart[]): void {
  [-1, 1].forEach((side) => {
    const suffix = side < 0 ? "R" : "L";
    const legX = side * 0.102;

    const legPivot = new THREE.Group();
    legPivot.name = `Leg_Hip_Pivot_${suffix}`;
    legPivot.position.set(legX, 0.48, 0);
    parent.add(legPivot);
    if (side > 0) pivots.legLPivot = legPivot; else pivots.legRPivot = legPivot;

    const legShorts = new THREE.Group();
    legShorts.name = `Shorts_Thigh_Group_${suffix}`;
    legPivot.add(legShorts);

    const legShort = new THREE.Mesh(new THREE.CylinderGeometry(0.104, 0.102, 0.165, 24), getOrCreateMat("MAT_Denim_Blue_Shorts", 0x264b73));
    legShort.name = `Shorts_Leg_${suffix}`;
    legShort.position.set(0, 0.415 - 0.48, 0);
    legShorts.add(legShort);
    tagCategory(legShorts, "shorts");

    const kneePivot = new THREE.Group();
    kneePivot.name = `Knee_Pivot_${suffix}`;
    kneePivot.position.set(0, 0.33 - 0.48, 0);
    legPivot.add(kneePivot);
    if (side > 0) pivots.kneeLPivot = kneePivot; else pivots.kneeRPivot = kneePivot;

    const kneeCap = new THREE.Mesh(new THREE.SphereGeometry(0.066, 18, 14), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    kneeCap.name = `Leg_Knee_Joint_${suffix}`;
    kneeCap.position.set(0, 0, 0);
    kneePivot.add(kneeCap);
    tagCategory(kneePivot, "limbs");

    const calf = new THREE.Mesh(new THREE.CylinderGeometry(0.068, 0.062, 0.175, 20), getOrCreateMat("MAT_Cream_Felt_Fur", 0xf1dfc7));
    calf.name = `Leg_Lower_Calf_${suffix}`;
    calf.position.set(0, 0.245 - 0.33, 0);
    kneePivot.add(calf);

    const footPivot = new THREE.Group();
    footPivot.name = `Foot_Ankle_Pivot_${suffix}`;
    footPivot.position.set(0, 0.14 - 0.33, 0);
    kneePivot.add(footPivot);
    if (side > 0) pivots.footLPivot = footPivot; else pivots.footRPivot = footPivot;

    const sock = new THREE.Mesh(new THREE.CylinderGeometry(0.069, 0.068, 0.068, 22), getOrCreateMat("MAT_Ribbed_White_Socks", 0xf2eadc));
    sock.name = `Sock_Ribbed_Cuff_${suffix}`;
    sock.position.set(0, 0.152 - 0.14, 0);
    footPivot.add(sock);

    const shoe = new THREE.Group();
    shoe.name = `Sneaker_Shoe_${suffix}`;
    shoe.position.set(0, -0.14, 0.035);
    shoe.rotation.y = side * 0.06;
    const sole = new THREE.Mesh(createRoundedBoxGeometry(0.152, 0.044, 0.265, 0.018, 4), getOrCreateMat("MAT_Sneaker_Cream_Rubber", 0xefe5d4));
    sole.name = `Sneaker_Rubber_Sole_${suffix}`;
    sole.position.set(0, 0.022, 0.01);
    shoe.add(sole);
    const upper = new THREE.Mesh(createRoundedBoxGeometry(0.142, 0.095, 0.21, 0.032, 4), getOrCreateMat("MAT_Sneaker_Blue_Canvas", 0x29527a));
    upper.name = `Sneaker_Denim_Upper_${suffix}`;
    upper.position.set(0, 0.078, -0.012);
    shoe.add(upper);
    footPivot.add(shoe);
    tagCategory(footPivot, "shoes");

    parts.push({
      id: side < 0 ? "P26" : "P25",
      name: `Leg_Hip_Pivot_${suffix}`,
      zone: "Upper Thighs",
      bone: `DEF_Thigh.${suffix}`,
      explode: [side * 0.22, -0.14, 0],
      desc: `${suffix} hip & denim shorts thigh + gold stitch cuff`,
      node: legPivot,
      visible: true,
      verified: true,
    });
    parts.push({
      id: side < 0 ? "P28" : "P27",
      name: `Knee_Pivot_${suffix}`,
      zone: "Knee & Shin",
      bone: `DEF_Shin.${suffix}`,
      explode: [side * 0.26, -0.26, 0.12],
      desc: `${suffix} articulated KNEE JOINT + plush lower calf`,
      node: kneePivot,
      visible: true,
      verified: true,
    });
    parts.push({
      id: side < 0 ? "P30" : "P29",
      name: `Foot_Ankle_Pivot_${suffix}`,
      zone: "Footwear",
      bone: `DEF_Foot.${suffix}`,
      explode: [side * 0.28, -0.40, 0.22],
      desc: `${suffix} ankle sock & blue denim sneaker + rubber toe`,
      node: footPivot,
      visible: true,
      verified: true,
    });
  });
}

// ---------- top-level assembly ----------

export interface BuiltRig {
  pivots: RigPivots;
  parts: RigPart[];
  root: any;
}

export function buildBunnyExplorerCharacter(species: Species = "bunny"): BuiltRig {
  const pivots: RigPivots = {
    root: null!, spinePivot: null!, pelvisPivot: null!, headPivot: null!,
    earLPivot: null!, earRPivot: null!, earTipLPivot: null!, earTipRPivot: null!,
    smileGroup: null!, eyelidUpperMeshes: [], eyelidLowerMeshes: [], eyeGazePivots: [], eyebrowMeshes: [],
    armLPivot: null!, armRPivot: null!, elbowLPivot: null!, elbowRPivot: null!,
    wristLPivot: null!, wristRPivot: null!, legLPivot: null!, legRPivot: null!,
    kneeLPivot: null!, kneeRPivot: null!, footLPivot: null!, footRPivot: null!,
    tailPivot: null!, tailMesh: null!, catTailMesh: null!,
    backpackGroup: null!, vestGroup: null!, torsoGroup: null!, shortsGroup: null!,
    scarfGroup: null!, satchelGroup: null!,
  };
  const parts: RigPart[] = [];

  const root = new THREE.Group();
  root.name = "Felt_Bunny_Explorer_RigRoot";
  pivots.root = root;

  const spinePivot = new THREE.Group();
  spinePivot.name = "Spine_Pivot";
  spinePivot.position.set(0, 0.56, 0);
  root.add(spinePivot);
  pivots.spinePivot = spinePivot;

  const upperBodyOffset = new THREE.Group();
  upperBodyOffset.name = "UpperBody_WorldSpace_Frame";
  upperBodyOffset.position.set(0, -0.56, 0);
  spinePivot.add(upperBodyOffset);

  buildHeadAssembly(upperBodyOffset, pivots, parts);
  buildFacialFeatures(pivots.headPivot!, pivots, parts);
  buildSpoonEars(pivots.headPivot!, pivots, parts);
  buildTorsoAndVest(upperBodyOffset, pivots, parts);
  buildArmsAndPaws(upperBodyOffset, pivots, parts);
  buildBackpack(upperBodyOffset, pivots, parts);

  buildPelvisAndShorts(root, pivots, parts);
  buildLegsAndFeet(root, pivots, parts);

  return { pivots, parts, root };
}
