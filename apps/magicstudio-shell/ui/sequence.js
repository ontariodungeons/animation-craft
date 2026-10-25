// Composite mode of the Animation view: images stacked on a canvas, each entering and leaving with
// a kind, duration, delay and ease — rendered by the engine (`sequence.render` → PhotoCraft layers
// → EffectCraft track). Subject mode animates one subject's rig; this mode arranges stills in time.
const seq = {
  project: null, canvas: [1080, 1080], preset: "1:1", duration: 4, fps: 30,
  background: "#101820", layers: [], selected: null, time: 0, playing: false,
  busy: false, result: null, tick: null, built: false,
};

const SEQ_PRESETS = {
  "1:1": [1080, 1080], "16:9": [1920, 1080], "9:16": [1080, 1920],
  "4:5": [1080, 1350], "3:2": [1500, 1000],
};
const SEQ_KINDS = ["none", "fade", "rise", "drop", "left", "right", "zoom_in", "zoom_out", "turn"];
const SEQ_EASES = ["smooth", "linear", "in", "out"];

function seqEase(name, u) {
  u = Math.max(0, Math.min(1, u));
  if (name === "linear") return u;
  if (name === "in") return u * u * u;
  if (name === "out") return 1 - (1 - u) ** 3;
  return u * u * (3 - 2 * u);
}

// The resting state and the departing state of one side, in the motion spec's units.
function seqSideValues(kind, [w, h]) {
  const rest = { position: [0, 0], rotation: 0, scale: [0, 0], opacity: 0 };
  const travel = 0.12;
  switch (kind) {
    case "fade": return { ...rest, opacity: -100 };
    case "rise": return { ...rest, position: [0, h * travel], opacity: -100 };
    case "drop": return { ...rest, position: [0, -h * travel], opacity: -100 };
    case "left": return { ...rest, position: [w * travel, 0], opacity: -100 };
    case "right": return { ...rest, position: [-w * travel, 0], opacity: -100 };
    case "zoom_in": return { ...rest, scale: [-25, -25], opacity: -100 };
    case "zoom_out": return { ...rest, scale: [25, 25], opacity: -100 };
    case "turn": return { ...rest, rotation: -18, opacity: -100 };
    default: return rest;
  }
}

function seqLerp(a, b, u) {
  return {
    position: [a.position[0] + (b.position[0] - a.position[0]) * u, a.position[1] + (b.position[1] - a.position[1]) * u],
    rotation: a.rotation + (b.rotation - a.rotation) * u,
    scale: [a.scale[0] + (b.scale[0] - a.scale[0]) * u, a.scale[1] + (b.scale[1] - a.scale[1]) * u],
    opacity: a.opacity + (b.opacity - a.opacity) * u,
  };
}

function seqOffsets(layer, t) {
  const rest = seqSideValues("none", seq.canvas);
  let value = rest;
  for (const [spec, entering] of [[layer.entry, true], [layer.exit, false]]) {
    if (!spec || spec.kind === "none") continue;
    const start = spec.delay ?? (entering ? 0 : Math.max(0, seq.duration - (spec.duration ?? 0.5)));
    const end = Math.min(seq.duration, start + (spec.duration ?? 0.6));
    if (t <= start) continue;
    const from = seqSideValues(spec.kind, seq.canvas);
    const part = seqLerp(from, rest, seqEase(spec.ease || "smooth", end > start ? (t - start) / (end - start) : 1));
    value = entering ? (t >= end ? rest : part) : part;
  }
  return value;
}

// ---- Surface ---------------------------------------------------------------------------------

// Composite mode borrows the Animation view's own chrome — coloured sections, play button,
// grid, fit and the time scrubber — and only supplies the stage contents and its controls.
function sequenceAttach(view) {
  animationShell();
  const stage = $("motion-stage");
  $("motion-canvas").hidden = true;
  stage.append(el("div", { class: "seq-canvas", id: "seq-canvas" }));
  document.querySelector(".motion-tools")?.prepend(motionButton("seq-add", "Add images to the sequence", "layers", seqAdd));
  const scrub = $("motion-scrub");
  scrub.max = String(seq.duration);
  scrub.oninput = (e) => { seqPause(); seq.time = Number(e.target.value); seqDraw(); };
  seq.built = true;
  seqRenderInspector();
  seqFit();
  seqDraw();
}

function seqCaption() {
  const caption = $("motion-caption");
  if (caption) caption.textContent = `Composite · ${seq.layers.length} image${seq.layers.length === 1 ? "" : "s"}`;
}

function seqField(label, control) {
  return el("label", { class: "seq-field" }, el("span", {}, label), control);
}

function seqSelect(label, options, value, change) {
  const select = el("select", { "aria-label": label, title: label },
    ...options.map((o) => el("option", { value: o }, o.replace("_", " "))));
  select.value = value;
  select.addEventListener("change", () => change(select.value));
  return seqField(label, select);
}

function seqRenderInspector() {
  const box = $("motion-inspector");
  if (!box) return;
  const layer = seq.layers.find((l) => l.id === seq.selected);
  box.replaceChildren(
    motionSection("Canvas", [
      seqSelect("Format", Object.keys(SEQ_PRESETS), seq.preset, (value) => {
        seq.preset = value;
        seq.canvas = SEQ_PRESETS[value].slice();
        seq.layers.forEach((l) => { l.center = [seq.canvas[0] / 2, seq.canvas[1] / 2]; });
        seqFit(); seqRenderInspector(); seqDraw();
      }),
      seqField("Background", el("input", { type: "color", value: seq.background, "aria-label": "Background colour",
        oninput: (e) => { seq.background = e.target.value; seqFit(); } })),
      motionRange("seq-seconds", "Seconds", 1, 20, 0.5, seq.duration, "s", (v) => {
        seq.duration = v;
        const scrub = $("motion-scrub");
        if (scrub) scrub.max = String(v);
      }),
    ], true, "motion-canvas-settings"),
    motionSection("Layers", (() => {
      const list = el("div", { class: "motion-layers", id: "seq-layers" });
      for (const l of [...seq.layers].reverse()) {
        const row = el("div", { class: "motion-layer", title: l.name },
          el("img", { src: convertFileSrc(l.file), alt: "" }),
          el("button", { class: "motion-layer-name", title: `Select ${l.name}`, onclick: () => { seq.selected = l.id; seqRenderInspector(); seqDraw(); } }, l.name),
          motionButton(`seq-hide-${l.id}`, `Remove ${l.name}`, "minus", () => seqRemove(l.id)));
        if (l.id === seq.selected) row.classList.add("selected");
        list.append(row);
      }
      if (!seq.layers.length) list.append(el("p", { class: "muted" }, "Add images to compose them in time."));
      seqCaption();
      return list;
    })(), true, "motion-layers"),
    layer ? motionSection(layer.name, [
      motionRange(`seq-scale-${layer.id}`, "Scale", 10, 300, 1, layer.scale, "%", (v) => { layer.scale = v; seqDraw(); }),
      ...["entry", "exit"].map((side) => {
        const spec = layer[side];
        return el("div", { class: "seq-side-block" },
          el("div", { class: "group-label" }, side === "entry" ? "Enter" : "Leave"),
          seqSelect("Kind", SEQ_KINDS, spec.kind, (v) => { spec.kind = v; seqDraw(); seqRenderInspector(); }),
          motionRange(`seq-${side}-duration-${layer.id}`, "Duration", 0.05, 3, 0.05, spec.duration, "s", (v) => { spec.duration = v; seqDraw(); }),
          motionRange(`seq-${side}-delay-${layer.id}`, "Delay", 0, 20, 0.05, spec.delay, "s", (v) => { spec.delay = v; seqDraw(); }),
          seqSelect("Ease", SEQ_EASES, spec.ease, (v) => { spec.ease = v; seqDraw(); }));
      }),
    ], true, "motion-selected-layer") : null,
    motionSection("Export & integrations", [
      el("button", { class: "start", id: "seq-export", title: "Render the sequence with EffectCraft", onclick: seqExport }, "Render"),
      el("div", { class: "why", id: "seq-status" }, seq.busy ? "Rendering…" : seq.result?.ok ? "Rendered" : ""),
      seq.result?.ok ? fileButton({ path: seq.result.file, name: seq.result.file.split("/").pop() }) : null,
    ], true, "motion-export-section"),
  );
}

function seqFit() {
  const canvas = $("seq-canvas");
  if (!canvas) return;
  canvas.style.setProperty("--seq-background", seq.background || "transparent");
}

function seqDraw() {
  const canvas = $("seq-canvas");
  if (!canvas) return;
  const [w, h] = seq.canvas;
  for (const layer of seq.layers) {
    let node = canvas.querySelector(`[data-layer="${layer.id}"]`);
    if (!node) {
      node = el("img", { "data-layer": layer.id, alt: layer.name, draggable: "false" });
      node.addEventListener("pointerdown", (e) => seqDrag(e, layer));
      canvas.append(node);
    }
    if (node.getAttribute("src") !== convertFileSrc(layer.file)) node.src = convertFileSrc(layer.file);
    const off = seqOffsets(layer, seq.time);
    const scale = Math.max(0, (layer.scale + off.scale[0]) / 100);
    node.style.setProperty("--seq-x", `${((layer.center[0] + off.position[0]) / w) * 100}%`);
    node.style.setProperty("--seq-y", `${((layer.center[1] + off.position[1]) / h) * 100}%`);
    node.style.setProperty("--seq-width", `${((layer.natural[0] * scale) / w) * 100}%`);
    node.style.setProperty("--seq-opacity", String(Math.max(0, Math.min(1, (100 + off.opacity) / 100))));
    node.style.setProperty("--seq-rotation", `${off.rotation}deg`);
    node.classList.toggle("selected", layer.id === seq.selected);
  }
  for (const node of [...canvas.children]) if (!seq.layers.some((l) => l.id === node.dataset.layer)) node.remove();
  canvas.classList.toggle("grid", motion.grid);
  const clock = $("motion-time");
  if (clock) clock.textContent = `${Math.floor(seq.time / 60)}:${(seq.time % 60).toFixed(3).padStart(6, "0")}`;
  const time = $("motion-scrub");
  if (time && document.activeElement !== time) time.value = String(Math.min(seq.time, seq.duration));
  seqCaption();
}

function seqDrag(event, layer) {
  event.preventDefault();
  seq.selected = layer.id;
  seqRenderInspector();
  const canvas = $("seq-canvas");
  const rect = canvas.getBoundingClientRect();
  const [w, h] = seq.canvas;
  const move = (e) => {
    layer.center = [
      Math.max(0, Math.min(w, ((e.clientX - rect.left) / rect.width) * w)),
      Math.max(0, Math.min(h, ((e.clientY - rect.top) / rect.height) * h)),
    ];
    seqDraw();
  };
  const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

function seqPlay() {
  seq.playing = true;
  let last = performance.now();
  const step = (now) => {
    if (!seq.playing) return;
    seq.time += (now - last) / 1000;
    last = now;
    if (seq.time > seq.duration) seq.time = 0;
    seqDraw();
    seq.tick = requestAnimationFrame(step);
  };
  seq.tick = requestAnimationFrame(step);
}

function seqPause() {
  seq.playing = false;
  cancelAnimationFrame(seq.tick);
}

async function seqAdd() {
  const files = await dialog.open({ title: "Add images", multiple: true, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }] });
  const list = Array.isArray(files) ? files : files ? [files] : [];
  for (const file of list) await seqAddFile(file);
}

async function seqAddFile(file) {
  const natural = await new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve([img.naturalWidth, img.naturalHeight]);
    img.onerror = () => resolve([0, 0]);
    img.src = convertFileSrc(file);
  });
  const id = `layer${seq.layers.length + 1}`;
  seq.layers.push({
    id, file, name: file.split("/").pop(), natural,
    center: [seq.canvas[0] / 2, seq.canvas[1] / 2], scale: 100,
    entry: { kind: "fade", duration: 0.6, delay: 0, ease: "smooth" },
    exit: { kind: "fade", duration: 0.5, delay: Math.max(0, seq.duration - 0.5), ease: "smooth" },
  });
  seq.selected = id;
  seqRenderInspector();
  seqDraw();
}

function seqRemove(id) {
  seq.layers = seq.layers.filter((l) => l.id !== id);
  if (seq.selected === id) seq.selected = seq.layers.at(-1)?.id ?? null;
  seqRenderInspector();
  seqDraw();
}

async function seqExport() {
  if (seq.busy) return;
  if (!seq.layers.length) { toast("Add an image first"); return; }
  seqPause();
  seq.busy = true;
  seqRenderInspector();
  try {
    const result = await motionCommand("sequence.render", {
      name: "sequence", canvas: seq.canvas, fps: seq.fps, duration: seq.duration, background: seq.background,
      layers: seq.layers.map((l) => ({ id: l.id, file: l.file, center: l.center, scale: l.scale, entry: l.entry, exit: l.exit })),
      stills: 4, video: true,
    });
    const out = result.export?.outputs?.[0];
    seq.result = { ok: Boolean(result.ok && out?.file), file: out?.file || "", validation: Boolean(result.export?.validation_ok) };
    toast(seq.result.ok ? "Sequence rendered" : "Render returned no track");
  } catch (error) {
    seq.result = { ok: false, file: "", error: String(error) };
    toast(String(error));
  } finally {
    seq.busy = false;
    seqRenderInspector();
  }
}
