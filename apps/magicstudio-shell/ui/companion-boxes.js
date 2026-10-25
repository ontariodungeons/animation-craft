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

// One number per state: the only thing a control has to reveal, and the only thing a pet reads.
//   0 none · 1 green in use · 2 yellow attention · 3 red problem
// Working is work in flight, so it is green and busy; yellow is only "needs you".
const SIGNAL = { idle: 0, ready: 1, working: 1, "needs-attention": 2, failed: 3 };
const SIGNAL_LABEL = { 0: "idle", 1: "in use", 2: "attention", 3: "problem" };

function createCompanionBoxes() {
  const boxes = new Map();
  const listeners = new Set();
  const jobs = new Map();
  const jobId = value => typeof value === "string" ? value : value?.id || null;
  let quiet = false;
  let changeSeq = 0;
  let draftFields = [];
  let panelRevision = 0;
  let panelSignature = "";
  let contractIssues = new Map(), contractSignature = "";
  const canonical = id => window.uiConfig?.elements?.[id]?.target || id;
  const currentProject = () => typeof state === "undefined" ? null : state.project ?? null;

  const elementRecords = new WeakMap();
  const instance = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  let nextIndex = 0, componentRevision = 0, componentSignature = "";
  function registerElement(node) {
    if (!node || !node.tagName || node.closest?.('script,style,head') || (node.namespaceURI === 'http://www.w3.org/2000/svg' && node.tagName.toLowerCase() !== 'svg')) return null;
    let record = elementRecords.get(node);
    if (!record) {
      record = { index: ++nextIndex };
      elementRecords.set(node, record);
      node.setAttribute('data-ui-index', String(record.index));
    }
    return record;
  }
  function inspect({ root = document.body, offset = 0, limit = 64, expected_instance = null, expected_revision = null } = {}) {
    if (!root) throw new Error("Unknown component region");
    if (expected_instance && expected_instance !== instance) throw new Error("Studio component instance changed; inspect again");
    const nodes = [root, ...root.querySelectorAll('*')].filter(n => registerElement(n));
    const items = nodes.map(node => {
      const record = registerElement(node), parent = registerElement(node.parentElement), rect = node.getBoundingClientRect(), computed = getComputedStyle(node);
      const configId = node.dataset.uiConfig || node.id, entry = window.uiConfig?.elements?.[configId], owner = node.closest('[data-companion-rail]')?.getAttribute('data-companion-rail') || node.closest('[data-box-id]')?.getAttribute('data-box-id');
      const live = owner ? get(owner) : null;
      const kind = node.tagName.toLowerCase(), editable = node.matches('input,select,textarea,[contenteditable=true]');
      const clickable = node.matches('button,a[href],[role=button],[role=tab],.chip,.proj,.object,.lyric');
      const data = node.type === 'checkbox' || node.type === 'radio' ? { checked: !!node.checked } :
        node.matches('input[type=number],input[type=range]') ? { value: node.value, min: node.min, max: node.max, step: node.step } :
        kind === 'select' ? { value: node.value, options: [...node.options].map(o => ({ value: o.value, label: o.label, disabled: o.disabled })) } : null;
      return { index: record.index, id: node.id || null, target: node.id || `@${record.index}`, parent_index: parent?.index ?? null, parent_id: node.parentElement?.id || null,
        tag: kind, component: entry?.component || kind, config: entry ? configId : null, rail: entry?.rail || null, container: owner ? canonical(owner) : null,
        rect: [rect.x, rect.y, rect.width, rect.height], visible: rect.width > 0 && rect.height > 0 && computed.visibility !== 'hidden' && !node.closest('[hidden],[aria-hidden=true]'),
        enabled: !node.disabled, selected: node.getAttribute('aria-selected') || node.getAttribute('aria-pressed') || null,
        signal: live ? boxSignal(live) : null, busy: live ? live.state === "working" : null,
        label: node.getAttribute('aria-label') || node.title || entry?.label || null, data,
        text: editable || node.closest('#agent-log,#agent-messages') ? null : [...(node.childNodes || [])].filter(n => n.nodeType === 3).map(n => n.textContent.trim()).filter(Boolean).join(' ').slice(0, 160),
        style: Object.fromEntries(['display','visibility','color','backgroundColor','fontSize','fontWeight','overflow','position'].map(key => [key, computed[key]])),
        commands: (node.dataset.commands || '').split(/\s+/).filter(Boolean), actions: editable ? ['ui.set','ui.key'] : clickable ? ['ui.click'] : [] };
    });
    const signature = JSON.stringify([currentProject(), items]);
    if (signature !== componentSignature) { componentSignature = signature; componentRevision++; }
    if (expected_revision !== null && Number(expected_revision) !== componentRevision) throw new Error("Component snapshot changed; restart pagination");
    offset = Math.max(0, Math.floor(Number(offset) || 0)); limit = Math.min(64, Math.max(1, Math.floor(Number(limit) || 64)));
    return { schema_version: 1, instance, revision: componentRevision, available: true, project: currentProject(), total: items.length, offset,
      items: items.slice(offset, offset + limit), next_offset: offset + limit < items.length ? offset + limit : null };
  }

  // One notification per change batch. Readers never poll.
  function notify() {
    if (quiet) return;
    quiet = true;
    queueMicrotask(() => {
      quiet = false;
      for (const node of document.querySelectorAll('[data-panel-indicator]')) {
        const box = get(node.getAttribute('data-panel-indicator'));
        if (box) paintIndicator(node, boxSignal(box));
      }
      for (const fn of listeners) {
        try { fn(); } catch (e) { console.warn(String(e)); }
      }
    });
  }

  const bounded = (map, key, value) => {
    if (map instanceof Map) map.set(key, value);
    else map.add(key);
    while (map.size > 64) map.delete(map.keys().next().value);
  };

  // Register (or update) a box. The id is stable; the project and context say
  // where the box lives, so an observation can be matched to it later.
  function register(id, { project = null, context = null, priority = 0, title = null, label = null, look = null, density = null } = {}) {
    if (!id) return null;
    const original = id;
    id = canonical(id);
    if (original !== id) {
      const entry = window.uiConfig?.elements?.[id];
      label = entry?.label ?? label; look = entry?.look ?? look; density = entry?.density ?? density;
    }
    priority = window.uiConfig?.elements?.[id]?.priority ?? priority;
    let box = boxes.get(canonical(id));
    if (!box) {
      box = {
        id,
        project,
        context,
        priority,
        title,
        label: label ?? title ?? null,
        look,
        density,
        open: false,
        busy: false,
        dirty: false,
        state: "idle",
        reason: null,
        request: null,
        job: null,
        resolution: null,
        source: null,
        observed_at: null,
        receipt: null,
      };
      boxes.set(id, box);
    } else {
      if (project != null && box.project !== project) {
        Object.assign(box, { project, context: null, resolution: null, state: "idle", busy: false, reason: null, request: null, job: null, source: null, attention: null, receipt: null, observed_at: null });
      }
      if (context != null) box.context = context;
      if (priority) box.priority = priority;
      if (title != null) box.title = title;
      if (label != null) box.label = label;
      if (look != null) box.look = look;
      if (density != null) box.density = density;
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
    box.seq = ++changeSeq;
    notify();
    return box;
  }

  // Readiness the page already renders. It may say a prerequisite is unmet;
  // it never means the box failed.
  // Readiness is what the page already renders. It may say a prerequisite is unmet, so it maps both
  // the legacy "needs" flag and a real box state; it never turns a control into a failure.
  function readiness(id, { state = null, reason = null, project = null } = {}) {
    const resolvedProject = project != null ? project : currentProject();
    const box = register(id, { project: resolvedProject });
    if (!box) return null;
    const next = state === "needs" ? "needs-attention" : BOX_STATES.includes(state) ? state : null;
    if (next && box.state !== next && box.state !== "working" && box.state !== "failed") {
      box.state = next;
      if (reason != null) box.reason = String(reason).slice(0, 180);
      box.source = "readiness";
      box.observed_at = Date.now();
      box.seq = ++changeSeq;
      notify();
    }
    return box;
  }

  // A command's receipt, kept beside the control it came from. Global Undo/Redo and Autosave read
  // this instead of every element growing its own save logic: one shape, written once.
  function recordReceipt(id, result = {}) {
    const box = boxes.get(canonical(id));
    if (!box) return null;
    const dirty = result.dirty;
    box.receipt = {
      undo: result.undo ?? null,
      revision: result.revision ?? result._ui_engine_revision ?? null,
      saved: result.saved ?? (dirty === false ? true : dirty === true ? false : null),
      command: result.command ?? null,
      at: Date.now(),
    };
    box.observed_at = Date.now();
    box.seq = ++changeSeq;
    notify();
    return box.receipt;
  }

  function event(event) {
      if (event.project !== state.project) return;
      const context = event.box ? event : capture(event.command, event.params);
      if (!context.box) return;
      const box = get(context.box), request = context.job ? context.request : event.request || context.request;
      if (box?.request && request && box.request !== request && (context.job || (event.phase !== "start" && box.state === "working"))) return;
      const result = event.result || {}, failed = event.phase === "error" || result.ok === false || result.state === "failed";
      const working = event.phase === "start" || ["submitted", "queued", "running", "cancelling"].includes(result.state);
      const job = jobId(result.job) || jobId(result.job_id) || context.job;
      if (job) bounded(jobs, `${event.project}:${job}`, { box: context.box, request });
      fact({ id: context.box, project: event.project, job, state: failed ? "failed" : working ? "working" : result.state === "cancelled" ? "idle" : "ready", request,
        reason: failed ? String(event.error || result.error || "Operation failed") : working ? "Working" : "Command completed", source: "command result" });
      if (event.phase === "success") recordReceipt(context.box, { ...result, command: event.command });
  }
  function capture(command, params = {}) {
      const action = typeof acFeedback === "undefined" ? null : acFeedback.action;
      const job = jobId(params.job) || jobId(params.job_id), owner = job ? jobs.get(`${state.project}:${job}`) : null;
      const node = document.getElementById(params?._control || action?.control) || [...document.querySelectorAll("[data-commands]")].find(n => (n.getAttribute("data-commands") || "").split(/\s+/).includes(command));
      const box = owner?.box || action?.box || node?.closest("[data-companion-rail]")?.getAttribute("data-companion-rail") || node?.closest("[data-box-id]")?.getAttribute("data-box-id") || null;
      return { command, params, box, job, request: owner?.request || params?._request_id || action?.request_id || null, project: state.project, view: state.view };
  }

  function attention(id, reason) {
    if (!boxes.has(canonical(id))) throw new Error("Unknown panel: " + id);
    if (reason !== null && (typeof reason !== "string" || !reason.trim())) throw new Error("Attention needs a reason");
    const box = register(id, { project: currentProject() });
    box.project = currentProject();
    box.attention = reason === null ? null : reason.trim().slice(0, 180);
    box.seq = ++changeSeq;
    box.observed_at = Date.now();
    notify();
  }
  const boxSignal = box => Math.max(SIGNAL[box.state] ?? 0, box.attention ? 2 : 0, contractIssues.has(box.id) ? 3 : 0);
  function get(id) {
    return boxes.get(canonical(id)) || null;
  }

  // The bounded snapshot the TUI (and any other reader) consumes. It says
  // where each observation came from and when it was seen, and it never
  // invents a failure.
  function snapshot({ project = null, limit = 64 } = {}) {
    const rows = [];
    for (const box of boxes.values()) {
      if (project && box.project && box.project !== project) continue;
      rows.push({
        id: box.id,
        label: box.label ?? null,
        look: box.look ?? null,
        busy: box.state === "working",
        project: box.project,
        context: box.context,
        state: box.state,
        signal: boxSignal(box),
        attention: box.attention ?? null,
        seq: box.seq ?? 0,
        reason: contractIssues.has(box.id) ? `UI contract: ${contractIssues.get(box.id).map(issue => issue.id).join(", ")}` : box.attention || box.reason,
        priority: box.priority,
        request: box.request,
        job: box.job,
        resolution: box.resolution,
        source: box.source,
        observed_at: box.observed_at,
        receipt: box.receipt,
      });
    }
    rows.sort((a, b) => (b.priority || 0) - (a.priority || 0) || a.id.localeCompare(b.id));
    return rows.slice(0, limit);
  }

  // One standard record per panel, plus the controls that belong to no panel.
  // The revision moves only when a flag or the unconfigured list really changes,
  // so the 1 s feedback heartbeat can re-send the same panels forever.
  function panels() {
    const bridge = typeof window !== "undefined" ? window.magicBridge : null;
    const unconfigured = bridge && typeof bridge.unconfigured === "function" ? bridge.unconfigured() : [];
    const nextContractSignature = JSON.stringify(unconfigured);
    if (nextContractSignature !== contractSignature) {
      contractSignature = nextContractSignature; contractIssues = new Map();
      for (const issue of unconfigured) {
        const target = canonical(issue.container || window.uiConfig?.contract_target);
        if (!boxes.has(target)) continue;
        if (!contractIssues.has(target)) contractIssues.set(target, []);
        contractIssues.get(target).push(issue);
      }
      notify();
    }
    const items = snapshot({ project: currentProject(), limit: Infinity });
    const signature = JSON.stringify([items.map(({ seq, observed_at, ...item }) => item), unconfigured]);
    if (signature !== panelSignature) {
      panelSignature = signature;
      panelRevision++;
    }
    return { revision: panelRevision, available: true, items, unconfigured, total_items: items.length, total_unconfigured: unconfigured.length, truncated: false };
  }

  function clear() {
    boxes.clear(); jobs.clear();
    notify();
  }

  // A control's public face: one number, and one light that cannot disagree with it.
  function signal(state) {
    return SIGNAL[state] ?? 0;
  }
  // The one indicator. The state picks the number; the variant changes the
  // shape only. The colour lives in CSS, as a token.
  // A light drawn for a panel is that panel's state: pass its id, and the light
  // and the record are one call, so the screen and the TUI cannot disagree.
  function paintIndicator(node, value) {
    node.dataset.signal = String(value);
    node.textContent = value === 0 ? (window.uiConfig?.signals?.[0]?.icon || "💤") : "";
    node.setAttribute("aria-label", SIGNAL_LABEL[value]);
  }
  function dot(state, variant = "", id = null) {
    if (id) show(id, state);
    const value = signal(state);
    const node = document.createElement("span");
    node.className = "dot";
    paintIndicator(node, id && boxes.has(canonical(id)) ? boxSignal(boxes.get(canonical(id))) : value);
    if (id) node.setAttribute("data-panel-indicator", id);
    node.hidden = !!id && window.uiConfig?.elements?.[id]?.indicator === "hidden";
    if (variant) node.dataset.variant = variant;
    node.setAttribute("role", "img");
    return node;
  }
  function show(id, state) {
    if (!BOX_STATES.includes(state)) return;
    const box = register(id);
    if (!box || box.state === state || box.state === "working" || box.state === "failed") return;
    box.state = state;
    box.source = "light";
    box.observed_at = Date.now();
    box.seq = ++changeSeq;
    notify();
  }
  function subscribe(fn) {
    listeners.add(fn);
    return () => listeners.delete(fn);
  }

  return { instance, registerElement, inspect, capture, event, attention, register, fact, readiness, recordReceipt, get, snapshot, panels, clear, states: BOX_STATES, signal, dot, subscribe };
}

// One shared registry, created before the app renders, so every control can
// join the contract the moment it is built.
window.uiRegistry = window.companionBoxes = createCompanionBoxes();

window.uiRegistry.subscribe(() => window.acFeedback?.report());
