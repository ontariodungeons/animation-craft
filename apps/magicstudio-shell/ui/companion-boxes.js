// The shared UI-box contract.
//
// Every control in the app belongs to a reusable box. A box has a stable id, the
// project and context it belongs to, and a state that describes what the app
// knows about it: idle, working, ready, needs-attention or failed. Boxes also
// carry the scent of the work in them -- reason, priority, the correlated
// request and job, and how a failure was resolved.
//
// Two rules matter more than the shape:
//
//  1. FACTS COME FROM EVENTS. A box only turns "working", "ready" or "failed"
//     when an actual command, job or render event says so. The readiness
//     `data-state` the page already renders may inform a prerequisite status,
//     but an empty box is NOT an error and never becomes "failed".
//  2. THE SCENT IS NOT DOM-ONLY. The same observation is published to the
//     bounded feedback snapshot the TUI already reads, so the app light, the
//     pet and the TUI all react to one semantic state instead of three guesses.
//     Frontend observation is marked unavailable or stale when the app is
//     closed; nothing is ever invented.
const BOX_STATES = ["idle", "working", "ready", "needs-attention", "failed"];

function createCompanionBoxes() {
  const boxes = new Map();
  const observed = new Map();

  const bounded = (map, key, value) => {
    if (map instanceof Map) map.set(key, value);
    else map.add(key);
    while (map.size > 64) map.delete(map.keys().next().value);
  };

  // Register (or update) a box. The id is stable; the project and context say
  // where the box lives, so an observation can be matched to it later.
  function register(id, { project = null, context = null, priority = 0, title = null } = {}) {
    if (!id) return null;
    let box = boxes.get(id);
    if (!box) {
      box = {
        id,
        project,
        context,
        priority,
        title,
        state: "idle",
        reason: null,
        request: null,
        job: null,
        resolution: null,
        source: null,
        observed_at: null,
      };
      boxes.set(id, box);
    } else {
      if (project != null) box.project = project;
      if (context != null) box.context = context;
      if (priority) box.priority = priority;
      if (title != null) box.title = title;
    }
    return box;
  }

  // The authoritative path: an event happened and says what the box is doing.
  // `fact` is what the engine or a correlated job reported.
  function fact({ id, project = null, context = null, state = null, reason = null, priority = null, request = null, job = null, resolution = null, source = "engine", observed_at = null } = {}) {
    const box = register(id, { project, context });
    if (!box) return null;
    if (state && BOX_STATES.includes(state)) box.state = state;
    if (reason != null) box.reason = String(reason).slice(0, 180);
    if (priority != null) box.priority = priority;
    if (request != null) box.request = request;
    if (job != null) box.job = job;
    if (resolution != null) box.resolution = resolution;
    box.source = source;
    box.observed_at = observed_at || Date.now();
    return box;
  }

  // The frontend's own view of a box (a draft, a render in flight, a UI
  // readiness hint). This is an OBSERVATION, not an engine fact, and it is
  // published as such so the TUI can tell the difference.
  function observe({ id, project = null, context = null, state = null, reason = null, priority = null, request = null, job = null } = {}) {
    const box = register(id, { project, context });
    if (!box) return null;
    if (state && BOX_STATES.includes(state)) {
      // A frontend observation may soften a fact, never assert a failure.
      if (state !== "failed" || box.source !== "engine") box.state = state;
    }
    if (reason != null) box.reason = String(reason).slice(0, 180);
    if (priority != null) box.priority = priority;
    if (request != null) box.request = request;
    if (job != null) box.job = job;
    box.source = "frontend";
    box.observed_at = Date.now();
    return box;
  }

  // Readiness the page already renders. It may say a prerequisite is unmet;
  // it never means the box failed.
  function readiness(id, { state = null, reason = null } = {}) {
    const box = register(id);
    if (!box) return null;
    if (state === "needs" && box.state !== "working" && box.state !== "failed") {
      box.state = "needs-attention";
      if (reason != null) box.reason = String(reason).slice(0, 180);
      box.source = "readiness";
      box.observed_at = Date.now();
    }
    return box;
  }

  // A box is resolved only by the event that matches its request or job.
  function resolve({ id = null, request = null, job = null, resolution = null, state = null, reason = null } = {}) {
    const candidates = [];
    if (id) {
      const box = boxes.get(id);
      if (box) candidates.push(box);
    }
    if (request) for (const box of boxes.values()) if (box.request === request) candidates.push(box);
    if (job) for (const box of boxes.values()) if (box.job === job) candidates.push(box);
    for (const box of candidates) {
      if (id && box.id !== id && !((request && box.request === request) || (job && box.job === job))) continue;
      if (resolution != null) box.resolution = resolution;
      if (state && BOX_STATES.includes(state)) box.state = state;
      if (reason != null) box.reason = String(reason).slice(0, 180);
      box.source = "engine";
      box.observed_at = Date.now();
    }
    return candidates;
  }

  function get(id) {
    return boxes.get(id) || null;
  }

  // The bounded snapshot the TUI (and any other reader) consumes. It says
  // where each observation came from and when it was seen, and it never
  // invents a failure.
  function snapshot({ project = null } = {}) {
    const rows = [];
    for (const box of boxes.values()) {
      if (project && box.project && box.project !== project) continue;
      rows.push({
        id: box.id,
        project: box.project,
        context: box.context,
        state: box.state,
        reason: box.reason,
        priority: box.priority,
        request: box.request,
        job: box.job,
        resolution: box.resolution,
        source: box.source,
        observed_at: box.observed_at,
      });
    }
    rows.sort((a, b) => (b.priority || 0) - (a.priority || 0) || a.id.localeCompare(b.id));
    return rows.slice(0, 64);
  }

  // When the app is closed the frontend cannot observe anything. Say so
  // instead of reporting a stale or invented state.
  function markUnavailable() {
    for (const box of boxes.values()) {
      if (box.source === "frontend") {
        box.source = "unavailable";
        box.state = "idle";
        box.reason = "The app was closed; this observation is stale.";
      }
    }
  }

  function clear() {
    boxes.clear();
    observed.clear();
  }

  return { register, fact, observe, readiness, resolve, get, snapshot, markUnavailable, clear, states: BOX_STATES };
}

// One shared registry, created before the app renders, so every control can
// join the contract the moment it is built.
window.companionBoxes = createCompanionBoxes();
