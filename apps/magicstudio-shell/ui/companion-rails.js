// Resolve configured panel slots from their actual layout. Missing targets never fall back to unrelated panels.
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
      const config = window.uiConfig?.rails?.[id];
      if (!config) continue;
      const accept = config.accept || "yes";
      const roles = config.roles || [];
      const priority = Number(config.priority) || 0;
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
      const config = window.uiConfig?.slots?.[id];
      if (!config) continue;
      const slot = {
        id,
        railId,
        element,
        target: config.target || id,
        priority: Number(config.priority) || 0,
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

  function resolve({ name, role = null, target = null } = {}) {
    if (!target) return null;
    const candidates = eligibleSlots({ name, role, target }).filter(entry => {
      const rect = measure(entry.slot.id);
      return railVisible(entry.rail.id) && rect && rect.x >= 0 && rect.y >= 0 &&
        rect.x + rect.width <= (globalThis.innerWidth ?? Infinity) && rect.y + rect.height <= (globalThis.innerHeight ?? Infinity);
    });
    candidates.sort((a, b) => b.slot.priority - a.slot.priority || b.rail.priority - a.rail.priority || a.slot.order - b.slot.order);
    if (!candidates.length) return null;
    const { rail, slot } = candidates[0];
    return { railId: rail.id, slotId: slot.id, target: slot.target, element: slot.element, rail };
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
    if (![rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) || rect.width <= 0 || rect.height <= 0) return null;
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
