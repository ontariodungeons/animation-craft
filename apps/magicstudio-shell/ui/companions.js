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
  let enabled = localStorage.getItem("magic.companions") !== "false";
  let project = null;
  let stage = null;
  let settleTimer = null;
  // per-pet timers live on pets[name]
  const nodes = {};
  const pets = {};
  const travelling = {};

  for (const name of ["sproket", "gaffer"]) {
    const node = el("pre", { id: `companion-${name}`, "aria-hidden": "true", inert: "" });
    nodes[name] = node;
    pets[name] = { pose: "rest", box: null, slot: null, signal: 0, timer: null, step: 0, lastBurstAt: 0 };
  }

  // ---- artwork ----------------------------------------------------------
  const useSprite = name => Boolean(sprites && sprites.has(name));

  function spriteElement(name) {
    const img = sprites.image(name, pets[name].pose || "rest");
    if (!img) return null;
    if (!nodes[name].sprite) {
      const holder = el("span", { id: `companion-${name}-sprite` });
      nodes[name].sprite = holder;
      // The holder is the pet's only child: without this the sprite is built but never painted.
      nodes[name].replaceChildren(holder);
    }
    const holder = nodes[name].sprite;
    if (holder.firstChild !== img) {
      holder.replaceChildren(img);
      holder.hidden = false;
    }
    return holder;
  }

  function frame() {
    for (const name of Object.keys(nodes)) {
      if (useSprite(name)) { spriteElement(name); continue; }
      const bank = frames?.frames?.[name];
      if (!bank?.length) continue;
      const text = bank[pets[name].step % bank.length];
      if (nodes[name].textContent !== text) nodes[name].textContent = text;
    }
  }

  // ---- placement --------------------------------------------------------
  // A box declares where a pet may stand. Missing slots remain explicit failures.
  function slotFor(name, boxId) {
    const config = window.uiConfig?.companions?.[name];
    if (!rails || !config) return null;
    return rails.resolve({ name, role: config.role, target: boxId || config.home });
  }

  // ONE compositor transform per retarget. Nothing is measured per frame.
  function travel(name, slot) {
    const node = nodes[name];
    if (!slot || !rails) return;
    const rect = rails.measure(slot.slotId);
    if (!rect) return;
    pets[name].slot = slot.slotId;
    if (!stage) {
      stage = el("div", { id: "companion-stage", "aria-hidden": "true", inert: "" });
      document.body.append(stage);
    }
    if (node.parentElement !== stage) stage.append(node);
    node.classList.add("companion-traveling");
    node.style.setProperty("--companion-position", `translate3d(${Math.round(rect.x + rect.width * Object.keys(pets).filter(p => pets[p].slot === slot.slotId).indexOf(name) / Object.keys(pets).filter(p => pets[p].slot === slot.slotId).length)}px, ${Math.round(rect.y)}px, 0)`);
    clearTimeout(travelling[name]);
    travelling[name] = setTimeout(() => {
      delete travelling[name];
      node.classList.remove("companion-traveling");
    }, 900);
  }

  function place(name) {
    const pet = pets[name];
    if (!enabled || !project || document.hidden) { nodes[name].hidden = true; return; }
    const slot = slotFor(name, pet.box);
    const rect = slot && rails.measure(slot.slotId);
    nodes[name].hidden = !rect;
    pet.placement_error = rect ? null : "No configured visible rail for this component";
    if (!rect) { pet.slot = null; return; }
    travel(name, slot);
  }

  // ---- the brain --------------------------------------------------------
  // A finite news burst: three poses, then still. Not a loop.
  function burst(name) {
    if (!enabled || document.hidden || media.matches || !useSprite(name)) return;
    const pet = pets[name];
    const now = Date.now();
    if (now - (pet.lastBurstAt || 0) < 1500) return;
    pet.lastBurstAt = now;
    // A burst is news, then the pet settles in the pose its signal asks for.
    const poses = ["work", "alert", pet.pose];
    clearTimeout(pet.timer);
    pet.step = 0;
    const tick = () => {
      pets[name].pose = poses[pet.step];
      frame();
      if (++pet.step >= poses.length) { pet.timer = null; return; }
      pet.timer = setTimeout(tick, 260);
    };
    pet.timer = setTimeout(tick, 260);
  }

  function react() {
    if (!boxes) return;
    if (!enabled) { for (const name of Object.keys(nodes)) nodes[name].hidden = true; return; }
    rails?.discover();
    const rows = boxes.panels().items.filter(row => row.signal > 0);
    const newestFirst = (a, b) => (b.seq ?? 0) - (a.seq ?? 0) || (b.observed_at ?? 0) - (a.observed_at ?? 0);
    const worst = rows.filter(row => row.signal >= 2).sort((a, b) => b.signal - a.signal || Number(!!b.attention) - Number(!!a.attention) || (b.priority || 0) - (a.priority || 0) || newestFirst(a, b))[0] || null;
    const recent = rows.filter(row => row.state === "working" || (row.source !== "readiness" && row.source !== "light" && Date.now() - row.observed_at < 8000));
    const newest = [...recent].sort(newestFirst)[0] || worst;
    clearTimeout(settleTimer);
    if (newest?.state === "ready") settleTimer = setTimeout(react, Math.max(1, 8000 - (Date.now() - newest.observed_at)));
    const next = {
      sproket: { box: worst?.id || null, signal: worst?.signal || 0, pose: worst ? (worst.signal === 3 ? "alert" : "work") : "rest" },
      gaffer: { box: newest?.id || null, signal: newest?.signal || 0, pose: newest ? (newest.state === "working" || newest.signal >= 2 ? "work" : "rest") : "asleep" },
    };
    for (const name of Object.keys(next)) {
      const pet = pets[name];
      const news = pet.box !== next[name].box || pet.pose !== next[name].pose;
      if (news && next[name].signal < 2) { clearTimeout(pet.timer); pet.timer = null; }
      Object.assign(pet, next[name]);
      if (news && next[name].signal >= 2) burst(name);
    }
    for (const name of Object.keys(nodes)) place(name);
    frame();
    if (typeof window.magicCompanionPaint === "function") window.magicCompanionPaint();
  }

  function reset() {
    clearTimeout(settleTimer);
    for (const name of Object.keys(pets)) clearTimeout(pets[name].timer);
    for (const name of Object.keys(pets)) pets[name] = { pose: "rest", box: null, slot: null, signal: 0, timer: null, step: 0, lastBurstAt: 0 };
    for (const name of Object.keys(nodes)) {
      nodes[name].hidden = true;
      nodes[name].style.removeProperty("--companion-position");
      delete nodes[name].sprite;
      nodes[name].replaceChildren();
    }
  }

  function sync() {
    if (project !== state.project) { project = state.project; reset(); }
    if (!enabled || media.matches || document.hidden) {
      clearTimeout(settleTimer);
      for (const pet of Object.values(pets)) { clearTimeout(pet.timer); pet.timer = null; }
      for (const name of Object.keys(travelling)) { clearTimeout(travelling[name]); delete travelling[name]; nodes[name].classList.remove("companion-traveling"); }
    }
    if (!enabled) { for (const name of Object.keys(nodes)) nodes[name].hidden = true; return; }
    react();
  }

  document.addEventListener?.("visibilitychange", sync);
  media.addEventListener?.("change", sync);
  const reposition = () => { rails?.invalidate(); for (const name of Object.keys(nodes)) place(name); };
  globalThis.addEventListener?.("resize", reposition);
  globalThis.addEventListener?.("scroll", reposition, true);

  // Change-driven: the page reports box changes; nothing here polls or scans per frame.
  boxes?.subscribe?.(() => react());
  sync();

  return {
    nodes,
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
        boxes: boxes ? boxes.panels().items : [],
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
    const rows = boxes.panels().items.filter(row => row.signal > 0);
    list.replaceChildren(...rows.slice(0, 6).map(row => {
      return el("li", {}, `${row.id}: signal ${row.signal}${row.reason ? ` — ${row.reason}` : ""}`);
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
