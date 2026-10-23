// Companion pets. They follow one thing: the signal a control already carries.
//
//   0 none · 1 green in use · 2 yellow attention · 3 red problem
//
// Sproket goes to the worst red/yellow and poses alert or work. Gaffer follows the most recently
// changed signal and wanders there. No command is inspected, no job is correlated and no per-control
// knowledge lives here: a box reports a state, the state maps to a number, the number picks a pose.
function createCompanions(frames, options = {}) {
  const sprites = options.sprites || null;
  const rails = options.rails || null;
  const boxes = options.boxes || window.companionBoxes || null;
  const media = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)") || { matches: false, addEventListener() {} };
  let enabled = localStorage.getItem("magic.companions") === "true";
  let project = null;
  let stage = null;
  let timer = null;
  let step = 0;
  const nodes = {};
  const pets = {};
  const anchors = {};
  const travelling = {};
  const ROLE = { gaffer: "artwork", sproket: "issue/chat" };

  for (const name of ["sproket", "gaffer"]) {
    const node = document.createElement("pre");
    node.id = `companion-${name}`;
    node.className = "ambient-companion";
    node.style.pointerEvents = "none";
    node.setAttribute("aria-hidden", "true");
    node.setAttribute("inert", "");
    nodes[name] = node;
    pets[name] = { pose: "rest", box: null, slot: null, signal: 0 };
  }

  // ---- artwork ----------------------------------------------------------
  const useSprite = name => Boolean(sprites && sprites.has(name));

  function spriteElement(name) {
    const img = sprites.image(name, pets[name].pose || "rest");
    if (!img) return null;
    if (!nodes[name].sprite) {
      const holder = document.createElement("span");
      holder.className = "companion-sprite";
      nodes[name].sprite = holder;
    }
    const holder = nodes[name].sprite;
    if (holder.firstChild !== img) {
      holder.replaceChildren(img);
      holder.hidden = false;
    }
    nodes[name].textContent = "";
    return holder;
  }

  function frame() {
    for (const name of Object.keys(nodes)) {
      if (useSprite(name)) { spriteElement(name); continue; }
      const bank = frames?.frames?.[name];
      if (!bank?.length) continue;
      const text = bank[step % bank.length];
      if (nodes[name].textContent !== text) nodes[name].textContent = text;
    }
  }

  // ---- placement --------------------------------------------------------
  // A box declares where a pet may stand. No slot declared means the pet keeps
  // its place and only changes pose.
  function slotFor(name, boxId) {
    if (!rails || !boxId) return null;
    const owner = [...document.querySelectorAll("[data-box-id]")].find(n => n.getAttribute("data-box-id") === boxId);
    const declared = owner?.getAttribute("data-companion-slot");
    if (declared) {
      rails.discover();
      return { slotId: declared, target: boxId };
    }
    return rails.resolve({ name, role: ROLE[name] || null, target: boxId, prefer: [boxId], current: pets[name].slot ? { slotId: pets[name].slot } : null });
  }

  // ONE compositor transform per retarget. Nothing is measured per frame.
  function travel(name, slot) {
    const node = nodes[name];
    if (!slot || !rails || pets[name].slot === slot.slotId) return;
    const rect = rails.measure(slot.slotId);
    if (!rect) return;
    pets[name].slot = slot.slotId;
    if (!stage) {
      stage = document.createElement("div");
      stage.className = "companion-stage";
      stage.setAttribute("aria-hidden", "true");
      stage.setAttribute("inert", "");
      document.body.append(stage);
    }
    if (node.parentElement !== stage) stage.append(node);
    node.classList.add("companion-traveling");
    node.style.transform = `translate3d(${Math.round(rect.x)}px, ${Math.round(rect.y)}px, 0)`;
    clearTimeout(travelling[name]);
    travelling[name] = setTimeout(() => {
      delete travelling[name];
      node.classList.remove("companion-traveling");
    }, 900);
  }

  function place(name) {
    const pet = pets[name];
    if (!enabled || !project || document.hidden) { nodes[name].hidden = true; return; }
    if (useSprite(name)) {
      nodes[name].hidden = false;
      travel(name, pet.box ? slotFor(name, pet.box) : null);
      return;
    }
    // ASCII fallback keeps its original inline behaviour: one anchor, no travel.
    const anchor = name === "gaffer"
      ? document.querySelector("#magic-image-toggle")
      : document.querySelector(".agent-title");
    if (!anchor || anchor.closest("[hidden],[aria-hidden=true]") || !anchor.getBoundingClientRect().width) {
      nodes[name].hidden = true;
      return;
    }
    if (anchors[name] !== anchor) {
      anchor.after(nodes[name]);
      anchors[name] = anchor;
      if (typeof placeInk === "function") placeInk();
    }
    nodes[name].hidden = false;
  }

  // ---- the brain --------------------------------------------------------
  // A finite news burst: three poses, then still. Not a loop.
  function burst(name) {
    if (!enabled || document.hidden || media.matches || !useSprite(name)) return;
    const poses = ["work", "alert", "rest"];
    clearTimeout(timer);
    step = 0;
    const tick = () => {
      pets[name].pose = poses[step];
      frame();
      if (++step >= poses.length) { timer = null; return; }
      timer = setTimeout(tick, 260);
    };
    timer = setTimeout(tick, 260);
  }

  function react() {
    if (!boxes) return;
    rails?.discover();
    const rows = boxes.snapshot({ project: state.project }).filter(row => row.signal > 0);
    const newestFirst = (a, b) => (b.seq ?? 0) - (a.seq ?? 0) || (b.observed_at ?? 0) - (a.observed_at ?? 0);
    const worst = rows.filter(row => row.signal >= 2).sort((a, b) => b.signal - a.signal || newestFirst(a, b))[0] || null;
    const newest = [...rows].sort(newestFirst)[0] || null;
    const next = {
      sproket: { box: worst?.id || null, signal: worst?.signal || 0, pose: worst ? (worst.signal === 3 ? "alert" : "work") : "rest" },
      gaffer: { box: newest?.id || null, signal: newest?.signal || 0, pose: newest ? (newest.signal >= 2 ? "work" : "rest") : "asleep" },
    };
    for (const name of Object.keys(next)) {
      const pet = pets[name];
      const news = pet.box !== next[name].box || pet.pose !== next[name].pose;
      Object.assign(pet, next[name]);
      if (news && next[name].signal >= 2) burst(name);
    }
    for (const name of Object.keys(nodes)) place(name);
    frame();
    if (typeof window.magicCompanionPaint === "function") window.magicCompanionPaint();
  }

  function reset() {
    for (const name of Object.keys(pets)) pets[name] = { pose: "rest", box: null, slot: null, signal: 0 };
    for (const name of Object.keys(nodes)) {
      nodes[name].hidden = true;
      nodes[name].style.transform = "";
      delete nodes[name].sprite;
      nodes[name].replaceChildren();
    }
  }

  function sync() {
    if (project !== state.project) { project = state.project; reset(); }
    if (!enabled) { for (const name of Object.keys(nodes)) nodes[name].hidden = true; return; }
    react();
  }

  document.addEventListener?.("visibilitychange", sync);
  media.addEventListener?.("change", sync);
  globalThis.addEventListener?.("resize", () => { rails?.invalidate(); for (const name of Object.keys(nodes)) place(name); });

  // Change-driven: the page reports box changes; nothing here polls or scans per frame.
  boxes?.subscribe?.(() => react());
  sync();

  return {
    nodes,
    // Kept so existing callers keep working; signals come from boxes now.
    event() {},
    capture(command, params = {}) { return { command, params, project: state.project, view: state.view }; },
    sync,
    setEnabled(value) {
      enabled = !!value;
      localStorage.setItem("magic.companions", String(enabled));
      sync();
    },
    snapshot() {
      return {
        enabled,
        project,
        reducedMotion: media.matches,
        hidden: document.hidden,
        sproket: { ...pets.sproket },
        gaffer: { ...pets.gaffer },
        boxes: boxes ? boxes.snapshot({ project }) : [],
      };
    },
  };
}

async function initCompanions() {
  const response = await fetch("companions.frames.json");
  const frames = response.ok ? await response.json() : { fps: 2, burst_ms: 1500, frames: {} };
  const sprites = typeof createCompanionSprites === "function" ? createCompanionSprites() : null;
  if (sprites) await sprites.load();
  const boxes = typeof createCompanionBoxes === "function" ? window.companionBoxes : null;
  const rails = typeof createCompanionRails === "function" ? createCompanionRails() : null;
  const controller = createCompanions(frames, { sprites, boxes, rails });
  window.magicCompanions = controller;
  window.magicCompanionBoxes = boxes;

  const list = document.getElementById("companion-state");
  const paint = () => {
    if (!list || !boxes) return;
    const rows = boxes.snapshot({ project: state.project }).filter(row => row.signal > 0);
    list.replaceChildren(...rows.slice(0, 6).map(row => {
      const item = document.createElement("li");
      item.textContent = `${row.id}: signal ${row.signal}${row.reason ? ` — ${row.reason}` : ""}`;
      return item;
    }));
  };
  paint();

  const setting = document.getElementById("ambient-companions");
  if (setting) {
    setting.checked = controller.snapshot().enabled;
    setting.defaultChecked = setting.checked;
    setting.addEventListener("change", () => {
      controller.setEnabled(setting.checked);
      setting.defaultChecked = setting.checked;
      paint();
    });
  }
  window.magicCompanionPaint = paint;
}

if (document.getElementById("ambient-companions")) initCompanions().catch(error => console.warn(String(error)));
