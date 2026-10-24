// The companion rail: ONE reusable horizontal component, used by every rail.
//
// A rail is not a bespoke lane per view. It is any element that declares
// itself a rail, and every rail -- the top bar, the footer, the docked
// workstation -- is the same contract with different contents:
//
//   <element data-companion-rail="dock"
//             data-companion-accept="yes|gaffer|sprocket"
//             data-companion-roles="artwork"
//             data-companion-priority="30">
//     <child data-companion-slot="chat"
//            data-companion-target="chat"
//            data-companion-priority="10"> ... </child>
//   </element>
//
// The companion resolver answers two questions, in this order:
//
//   1. ELIGIBILITY -- may this companion stand here at all?
//      The rail must accept it and serve its role, and the slot must exist.
//   2. PRIORITY -- among the eligible slots, which one?
//      Explicit slot priority, then rail priority, then the caller's ordered
//      preference (current target, last meaningful operation), then DOM order.
//      The last step makes the answer deterministic, so the same inputs always
//      pick the same slot and the pets never flap between two equals.
//
// A rail only ever answers where a companion may PHYSICALLY stand. It never
// decides what the companion is doing -- that belongs to the state machine,
// which preserves the unresolved-issue pin (Sproket) and the latest-work
// behaviour (Gaffer). Priority picks a landing place; it never fabricates
// activity.
//
// GEOGRAPHY. The Studio is a separate webview painted OVER the project view,
// so no z-index in this document can put a companion above it. Only the truly
// visible strips -- the top bar, the footer, and the docked workstation -- can
// host a rail. A companion whose work lives in the covered content resolves to
// the nearest declared visible boundary slot instead.
//
// MEASUREMENT. Rects are measured from the real layout, but only when they
// can have changed: on a scoped resize/scroll/layout event, and when a
// companion retargets. Nothing is read per frame.
function createCompanionRails({ root = document } = {}) {
  const rails = new Map();
  const slots = new Map();
  const measured = new Map();
  let generation = 0;

  // ---- discovery --------------------------------------------------------
  function discover() {
    rails.clear();
    slots.clear();
    measured.clear();
    generation++;
    const railElements = root.querySelectorAll("[data-companion-rail]");
    for (const element of railElements) {
      const id = element.getAttribute("data-companion-rail");
      if (!id) continue;
      const accept = (element.getAttribute("data-companion-accept") || "yes").trim();
      const roles = (element.getAttribute("data-companion-roles") || "").split(/[\s,]+/).filter(Boolean);
      const priority = Number(element.getAttribute("data-companion-priority") || 0) || 0;
      const rail = {
        id,
        element,
        // "yes" accepts everyone, otherwise it is a list of companion names.
        acceptAll: accept === "yes",
        accept: accept === "yes" ? [] : accept.split(/[\s,]+/).filter(Boolean),
        roles,
        priority,
        slots: [],
      };
      rails.set(id, rail);
    }
    for (const element of root.querySelectorAll("[data-companion-slot]")) {
      const id = element.getAttribute("data-companion-slot");
      if (!id) continue;
      const railElement = element.closest("[data-companion-rail]");
      const railId = railElement ? railElement.getAttribute("data-companion-rail") : null;
      if (!railId || !rails.has(railId)) continue;
      const slot = {
        id,
        railId,
        element,
        target: element.getAttribute("data-companion-target") || id,
        priority: Number(element.getAttribute("data-companion-priority") || 0) || 0,
        order: slots.size,
      };
      slots.set(id, slot);
      rails.get(railId).slots.push(slot);
    }
    return rails;
  }

  // ---- eligibility ------------------------------------------------------
  function railAccepts(rail, name, role) {
    if (rail.acceptAll) {
      if (!rail.roles.length) return true;
      return !role || rail.roles.includes(role);
    }
    return rail.accept.includes(name);
  }

  function eligibleSlots({ name, role, target = null }) {
    const found = [];
    for (const rail of rails.values()) {
      if (!railAccepts(rail, name, role)) continue;
      for (const slot of rail.slots) {
        if (target && slot.target !== target) continue;
        found.push({ rail, slot });
      }
    }
    return found;
  }

  // ---- selection --------------------------------------------------------
  // `prefer` is an ordered list of slot ids or targets: the current target
  // first, then the last meaningful operation. It only breaks ties.
  function resolve({ name, role = null, target = null, prefer = [], current = null } = {}) {
    // Eligible means: the rail accepts this companion, it serves the role, the
    // rail is on screen, and the slot exists.
    const candidates = eligibleSlots({ name, role }).filter(entry => railVisible(entry.rail.id));
    if (!candidates.length) return null;
    const preferIndex = entry => {
      for (let i = 0; i < prefer.length; i++) {
        const wanted = prefer[i];
        if (wanted === entry.slot.id || wanted === entry.slot.target) return i;
      }
      return prefer.length;
    };
    // A slot that serves the wanted target always outranks one that does not.
    // A content target with no slot of its own therefore resolves to the
    // nearest declared visible boundary slot instead of nowhere.
    const matches = entry => (target ? entry.slot.target === target : true);
    const staying = current ? candidates.find(entry => entry.slot.id === current.slotId) : null;
    const ranked = candidates
      .map(entry => ({ entry, prefer: preferIndex(entry), match: matches(entry) }))
      .sort((a, b) => {
        if (a.match !== b.match) return a.match ? -1 : 1;
        if (staying) {
          const aStay = a.entry.slot.id === staying.slot.id;
          const bStay = b.entry.slot.id === staying.slot.id;
          if (aStay !== bStay) return aStay ? -1 : 1;
        }
        if (a.entry.slot.priority !== b.entry.slot.priority) return b.entry.slot.priority - a.entry.slot.priority;
        if (a.entry.rail.priority !== b.entry.rail.priority) return b.entry.rail.priority - a.entry.rail.priority;
        if (a.prefer !== b.prefer) return a.prefer - b.prefer;
        return a.entry.slot.order - b.entry.slot.order;
      });
    const best = ranked[0].entry;
    return { railId: best.rail.id, slotId: best.slot.id, target: best.slot.target, element: best.slot.element, rail: best.rail };
  }

  // ---- geometry ---------------------------------------------------------
  // Measured from the real layout, cached, and invalidated only when the
  // layout could have changed. No per-frame reads.
  function measure(slotId) {
    const slot = slots.get(slotId);
    if (!slot) return null;
    const cached = measured.get(slotId);
    if (cached && cached.generation === generation) return cached.rect;
    const rect = slot.element.getBoundingClientRect();
    const value = { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
    measured.set(slotId, { generation, rect: value });
    return value;
  }

  function invalidate() {
    measured.clear();
    generation++;
  }

  // A rail is visible when it has a real box on screen.
  function railVisible(railId) {
    const rail = rails.get(railId);
    if (!rail) return false;
    const rect = rail.element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }

  return {
    discover,
    invalidate,
    measure,
    railVisible,
    resolve,
    eligibleSlots,
    get railCount() {
      return rails.size;
    },
    get slotCount() {
      return slots.size;
    },
    rails,
    slots,
  };
}
