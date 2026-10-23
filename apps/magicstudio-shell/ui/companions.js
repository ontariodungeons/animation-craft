// Companion pets.
//
// This file keeps the controller that was already here and correct: project and
// job correlation, a bounded 64-entry dedupe, and error resolution that only a
// matching event may resolve. The unresolved-issue pin (Sproket) and the
// latest-work behaviour (Gaffer) are unchanged.
//
// What changes is how a companion is DRAWN and WHERE it stands:
//
//   * The ASCII frames remain the fallback when sprite artwork is unavailable.
//   * With artwork, a companion travels to a slot on a rail. Every rail is the
//     same reusable component (see companion-rails.js); a rail answers where a
//     companion may physically stand and never what it is doing.
//   * Travel is a finite compositor transform, and a news burst is a finite
//     pose sequence. Both stop immediately when the pets are off, the window
//     is hidden, or reduced motion is asked for.
//
// Geometry is read only when it can have changed -- on a scoped resize, scroll
// or layout event, and when a companion retargets -- never per frame.
function createCompanions(frames, options = {}) {
  const sprites = options.sprites || null;
  const rails = options.rails || null;
  // One shared box registry: the app's controls register themselves as they
  // are built, so the companion and the TUI read the same observation.
  const boxes = options.boxes || window.companionBoxes || null;
  const media = matchMedia("(prefers-reduced-motion: reduce)");
  const seen = new Set();
  const jobs = new Map();
  let enabled = localStorage.getItem("magic.companions") === "true";
  let project = null;
  let timer = null;
  let step = 0;
  let sequence = 0;
  let stage = null;
  const nodes = {};
  const pets = {};
  const anchors = {};
  const travelling = {};

  const home = name => ({
    destination: name === "gaffer" ? "artwork" : "chat",
    status: "home",
    command: null,
    request: null,
    reason: "Home",
    key: null,
    slot: null,
    box: null,
  });

  // The role a companion plays. It decides which rails may host it.
  const ROLE = { gaffer: "artwork", sproket: "issue/chat" };

  for (const name of ["sproket", "gaffer"]) {
    const node = document.createElement("pre");
    node.id = `companion-${name}`;
    node.className = "ambient-companion";
    node.style.pointerEvents = "none";
    node.setAttribute("aria-hidden", "true");
    node.setAttribute("inert", "");
    nodes[name] = node;
    pets[name] = home(name);
  }

  const bounded = (map, key, value) => {
    if (map instanceof Map) map.set(key, value);
    else map.add(key);
    while (map.size > 64) map.delete(map.keys().next().value);
  };

  const tab = view => [...document.querySelectorAll(".tab[data-view]")].find(n => n.dataset.view === view);
  const anchor = destination =>
    destination === "chat"
      ? document.querySelector(".agent-title")
      : destination === "artwork"
        ? document.querySelector("#magic-image-toggle")
        : tab(destination);
  const visible = name => {
    const a = anchors[name];
    return (
      enabled &&
      !!project &&
      !document.hidden &&
      !!a &&
      a.getBoundingClientRect().width > 0 &&
      !a.closest?.("[hidden],[aria-hidden=true]")
    );
  };

  // ---- artwork ----------------------------------------------------------
  // Prefer a sprite; fall back to the shipped ASCII frames when the artwork
  // manifest is missing or unreadable. Both are valid states.
  function useSprite(name) {
    return Boolean(sprites && sprites.has(name));
  }

  function spriteElement(name) {
    const pose = pets[name].pose || "rest";
    const img = sprites.image(name, pose);
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

  function frame(index) {
    for (const name of Object.keys(nodes)) {
      if (useSprite(name)) {
        spriteElement(name);
        continue;
      }
      const text = frames.frames[name][index % frames.frames[name].length];
      if (visible(name) && nodes[name].textContent !== text) nodes[name].textContent = text;
    }
  }

  function stop() {
    clearTimeout(timer);
    timer = null;
    step = 0;
    frame(0);
  }

  // ---- placement --------------------------------------------------------
  // A rail answers the physical landing. Resolution is eligibility first, then
  // priority, then a deterministic tie-break, so the answer never flaps.
  function resolveSlot(name) {
    if (!rails) return null;
    const pet = pets[name];
    const prefer = [];
    if (pet.destination) prefer.push(pet.destination);
    if (pet.lastTarget) prefer.push(pet.lastTarget);
    const current = pet.slot ? { slotId: pet.slot } : null;
    return rails.resolve({
      name,
      role: ROLE[name] || null,
      target: pet.destination || null,
      prefer,
      current,
    });
  }

  // Travel is a finite compositor transform: ONE transform change, animated by
  // the compositor. Nothing is measured per frame, and the timeout only clears
  // the travelling state -- it is cancelled the moment the pet retargets.
  function travel(name, slot) {
    const node = nodes[name];
    if (!slot) return;
    const rect = rails.measure(slot.slotId);
    if (!rect) return;
    const pet = pets[name];
    pet.slot = slot.slotId;
    pet.lastTarget = slot.target;
    if (!stage) {
      stage = document.createElement("div");
      stage.className = "companion-stage";
      stage.setAttribute("aria-hidden", "true");
      stage.setAttribute("inert", "");
      document.body.append(stage);
    }
    if (node.parentElement !== stage) stage.append(node);
    node.classList.add("companion-traveling");
    // The pet stands ON the slot: its bottom-left corner sits there.
    node.style.transform = `translate3d(${Math.round(rect.x)}px, ${Math.round(rect.y)}px, 0)`;
    clearTimeout(travelling[name]);
    travelling[name] = setTimeout(() => {
      delete travelling[name];
      node.classList.remove("companion-traveling");
    }, 900);
  }

  function place(name) {
    const pet = pets[name];
    // Artwork pets travel on a rail. A pet whose artwork is unavailable keeps
    // the shipped inline ASCII behaviour exactly as it was.
    if (useSprite(name)) {
      const slot = resolveSlot(name);
      if (slot) {
        travel(name, slot);
        nodes[name].hidden = false;
        return;
      }
      nodes[name].hidden = !visible(name);
      return;
    }
    const a = anchor(pet.destination);
    if (a && anchors[name] !== a) {
      a.after(nodes[name]);
      anchors[name] = a;
      if (typeof placeInk === "function") placeInk();
    }
    const hidden = !visible(name);
    if (nodes[name].hidden !== hidden) nodes[name].hidden = hidden;
  }

  function render() {
    for (const name of Object.keys(pets)) place(name);
  }

  function sync() {
    if (project !== state.project) {
      stop();
      project = state.project;
      seen.clear();
      jobs.clear();
      for (const name of Object.keys(pets)) pets[name] = home(name);
      if (boxes) {
        boxes.clear();
        for (const name of Object.keys(pets)) {
          boxes.register(`companion.${name}`, { project: state.project, context: ROLE[name] || null });
        }
      }
    }
    if (rails) rails.discover();
    render();
    if (!enabled || document.hidden || media.matches || !Object.keys(nodes).some(visible)) stop();
    if (timer === null) frame(0);
  }

  // ---- a finite news burst ----------------------------------------------
  // Three poses and back to rest. It is a bounded sequence, not a loop.
  const BURST_POSES = { gaffer: ["work", "alert", "rest"], sproket: ["work", "alert", "rest"] };
  function burst() {
    clearTimeout(timer);
    timer = null;
    step = 0;
    render();
    if (!enabled || document.hidden || media.matches || !Object.keys(nodes).some(visible)) {
      stop();
      return;
    }
    if (!useSprite("sproket") && !useSprite("gaffer")) {
      // ASCII fallback keeps the original two-frame blink.
      const tick = () => {
        step++;
        if (step >= 3) {
          timer = null;
          step = 0;
          frame(0);
          return;
        }
        frame(step);
        timer = setTimeout(tick, 500);
      };
      timer = setTimeout(tick, 500);
      return;
    }
    const tick = () => {
      const poses = BURST_POSES.sproket;
      const index = step % poses.length;
      for (const name of Object.keys(pets)) {
        pets[name].pose = poses[index];
        frame(0);
      }
      step++;
      if (step >= poses.length + 1) {
        timer = null;
        step = 0;
        for (const name of Object.keys(pets)) pets[name].pose = "rest";
        frame(0);
        return;
      }
      timer = setTimeout(tick, 260);
    };
    timer = setTimeout(tick, 260);
  }

  // ---- events -----------------------------------------------------------
  const noise = id =>
    !id ||
    /^(agent\.|ui\.|center\.|project\.(open|close|root|recent)|motion\.(sample|tracks)|blender\.(actions|prepare))/.test(id) ||
    /(^|[.])(status|list|read|state|log|session|models|feedback|preview|inspect|probe|capabilities|get)([.]|$)|\.(get_|list_|read_)/.test(id);
  const keyOf = e =>
    JSON.stringify([e.command, ...["character", "asset", "part", "track", "scene", "placement", "job", "path"].map(k => e.params?.[k] ?? null)]);

  function event(input) {
    // A late completion may never switch the active project or erase its current explanation.
    if (input.project !== state.project || (input.result?.project && input.result.project !== state.project)) return;
    let e = input;
    if (!noise(e.command) && !enabled) return;
    if (!enabled || !state.project) return;
    if (project !== state.project) sync();
    if (/\.status$/.test(e.command) && e.result?.job) {
      let previous = jobs.get(e.result.job);
      if (!previous) {
        if (["running", "cancelling"].includes(e.result.state))
          bounded(jobs, e.result.job, {
            event: { ...e, command: e.command.replace(/status$/, "start"), request: e.result.job },
            state: e.result.state,
          });
        return;
      }
      if (previous.state === e.result.state) return;
      previous.state = e.result.state;
      if (!["completed", "failed", "cancelled"].includes(e.result.state)) return;
      e = {
        ...previous.event,
        request: `${previous.event.request}:${e.result.job}:${e.result.state}`,
        phase: e.result.state === "failed" ? "error" : "success",
        result: e.result,
        error: e.result.error || e.result.detail,
      };
    } else if (noise(e.command)) {
      // Chat can itself fail, but asking why must not resolve or relocate an existing pin.
      if (e.command !== "agent.send") return;
      if (e.phase === "error") {
        if (["failed", "blocked"].includes(pets.sproket.status)) return;
      } else if (e.phase !== "success" || pets.sproket.command !== "agent.send") return;
    }
    if (!tab(e.view)) return;
    const signature = JSON.stringify([
      e.project,
      e.command === "image.generate" && e.phase === "success" ? e.result?.asset_id || e.request : e.request,
      e.command,
      e.phase,
      e.result?.state,
    ]);
    if (seen.has(signature)) return;
    bounded(seen, signature);
    sync();
    if (e.result?.job && e.command.endsWith(".start"))
      bounded(jobs, e.result.job, { event: e, state: e.result.state });

    const failed = e.phase === "error" || e.result?.ok === false || ["failed", "blocked"].includes(e.result?.state);
    const pending = ["submitted", "running", "cancelling"].includes(e.result?.state);
    const key = keyOf(e);
    const status = failed
      ? e.result?.state === "blocked"
        ? "blocked"
        : "failed"
      : e.phase === "start" || pending
        ? "working"
        : "completed";

    // The box contract: a fact from an actual event, never a guess.
    if (boxes) {
      const boxId = e.view ? `view.${e.view}` : `command.${e.command}`;
      const fact = {
        id: boxId,
        project: state.project,
        context: e.view || null,
        state: failed ? "failed" : pending || e.phase === "start" ? "working" : "ready",
        reason: failed ? String(e.error || e.result?.error || `${status}: ${e.command}`).slice(0, 180) : `${status}: ${e.command}`,
        priority: failed ? 30 : pending ? 20 : 10,
        request: e.request || null,
        job: e.result?.job || null,
        resolution: failed ? (e.result?.state === "blocked" ? "blocked" : "failed") : "completed",
        source: "engine",
      };
      boxes.fact(fact);
    }

    if (e.command !== "agent.send")
      pets.gaffer = {
        ...pets.gaffer,
        destination: e.command === "image.generate" && !failed && e.phase !== "start" ? "artwork" : e.view,
        status,
        command: e.command,
        request: e.request,
        reason: `${status}: ${e.command}`,
        key,
      };
    if (failed)
      pets.sproket = {
        ...pets.sproket,
        destination: e.view,
        status,
        command: e.command,
        request: e.request,
        reason: String(e.error || e.result?.error || `${status}: ${e.command}`).slice(0, 180),
        key,
      };
    else if (e.phase !== "start" && !pending && pets.sproket.key === key)
      pets.sproket = { ...home("sproket"), status: "resolved", command: e.command, request: e.request, reason: `Resolved: ${e.command}` };

    render();
    burst();
    if (typeof window.magicCompanionPaint === "function") window.magicCompanionPaint();
  }

  document.addEventListener("visibilitychange", sync);
  media.addEventListener("change", sync);

  // Scoped invalidation: the geometry is only re-read when the layout could
  // have moved. No per-frame work, and nothing runs while nothing happens.
  if (typeof ResizeObserver === "function" && rails) {
    const observer = new ResizeObserver(() => {
      rails.invalidate();
      render();
    });
    const observeRails = () => {
      for (const rail of rails.rails.values()) observer.observe(rail.element);
    };
    observeRails();
    addEventListener("resize", () => {
      rails.invalidate();
      render();
    });
  }

  sync();

  return {
    nodes,
    event,
    sync,
    capture(command, params = {}) {
      const e = {
        command,
        params,
        project: state.project,
        view: state.view,
        request: params._request_id || `ambient-${Date.now()}-${++sequence}`,
      };
      event({ ...e, phase: "start" });
      return e;
    },
    setEnabled(value) {
      enabled = !!value;
      localStorage.setItem("magic.companions", String(enabled));
      sync();
    },
    snapshot() {
      return JSON.parse(
        JSON.stringify({
          enabled,
          project,
          reducedMotion: media.matches,
          hidden: document.hidden,
          animating: timer !== null,
          gaffer: pets.gaffer,
          sproket: pets.sproket,
          dedupe: seen.size,
          jobs: jobs.size,
          boxes: boxes ? boxes.snapshot({ project: state.project }) : [],
        })
      );
    },
  };
}

async function initCompanions() {
  const framesResponse = await fetch("companions.frames.json");
  const frames = framesResponse.ok ? await framesResponse.json() : { fps: 2, burst_ms: 1500, frames: {} };
  const sprites = typeof createCompanionSprites === "function" ? createCompanionSprites() : null;
  if (sprites) await sprites.load();
  const boxes = typeof createCompanionBoxes === "function" ? window.companionBoxes : null;
  const rails = typeof createCompanionRails === "function" ? createCompanionRails() : null;
  const controller = createCompanions(frames, { sprites, boxes, rails });
  window.magicCompanions = controller;
  window.magicCompanionBoxes = boxes;

  // The pets are decorative and hidden from assistive technology, so the same
  // state is also written as plain text. Nothing is conveyed by the artwork
  // alone.
  const list = document.getElementById("companion-state");
  const paint = () => {
    if (!list || !boxes) return;
    const rows = boxes.snapshot({ project: state.project }).filter(row => row.state !== "idle");
    list.replaceChildren(
      ...rows.slice(0, 6).map(row => {
        const item = document.createElement("li");
        item.textContent = `${row.id}: ${row.state}${row.reason ? ` — ${row.reason}` : ""}`;
        return item;
      })
    );
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
