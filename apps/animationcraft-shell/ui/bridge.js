// The page's half of the `--control` bridge (src/bridge.rs): what the CLI and MCP see and click.
// An element's id is its DOM id, else its aria-label, else its visible text; a region is a DOM id
// or a CSS selector.
const CLICKABLE = "button, [role=button], [role=tab], a[href], input, select, textarea, .chip, .proj, .object, .lyric";

function visible(node) {
  const r = node.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && getComputedStyle(node).visibility !== "hidden" && !node.closest("[aria-hidden=true], [hidden]");
}

function regionNode(region) {
  if (!region) return document.body;
  return document.getElementById(region) || document.querySelector(region);
}

function idOf(node) {
  return node.id || node.getAttribute("aria-label") || (node.innerText || node.value || node.placeholder || "").trim().split("\n")[0].slice(0, 60);
}

function find(target) {
  if (!target) throw new Error("which element? pass its id");
  const byId = document.getElementById(target);
  if (byId) return byId;
  const nodes = [...document.querySelectorAll(CLICKABLE)].filter(visible);
  const hit = nodes.find((n) => idOf(n) === target) || nodes.find((n) => idOf(n).toLowerCase().includes(String(target).toLowerCase()));
  if (hit) return hit;
  try { const q = document.querySelector(target); if (q) return q; } catch { /* not a selector */ }
  throw new Error(`no element "${target}"; ui.elements lists them`);
}

window.acBridge = {
  state() {
    return {
      animation: typeof motion === "undefined" ? null : { character: motion.character, track: motion.track, rest: motion.rest, time: motion.time, playing: motion.playing, selected_layer: motion.selected },
      project: state.project, view: document.body.classList.contains("at-home") ? "home" : state.view, open_block: state.open,
      storyboard_page: state.page + 1, agent_open: agent.open, agent_session: agent.session,
      studio: serverUrl?.() || null, toast: $("toast").classList.contains("show") ? $("toast").textContent : null,
    };
  },
  elements(region) {
    const root = regionNode(region);
    if (!root) throw new Error(`no region "${region}"`);
    return [...root.querySelectorAll(CLICKABLE)].filter(visible).map((n) => {
      const r = n.getBoundingClientRect();
      return { id: idOf(n), kind: n.getAttribute("role") || n.tagName.toLowerCase(), label: n.title || n.getAttribute("aria-label") || "",
        rect: [Math.round(r.left), Math.round(r.top), Math.round(r.width), Math.round(r.height)] };
    });
  },
  click(target) {
    const node = find(target);
    node.scrollIntoView({ block: "nearest" });
    node.click();
    return { clicked: idOf(node) };
  },
  set(target, value) {
    const node = find(target);
    node.value = value;
    node.dispatchEvent(new Event("input", { bubbles: true }));
    node.dispatchEvent(new Event("change", { bubbles: true }));
    return { set: idOf(node), value };
  },
  key(key, target) {
    const node = target ? find(target) : document.activeElement || document.body;
    const [mods, k] = [key.split("+").slice(0, -1).map((m) => m.toLowerCase()), key.split("+").pop()];
    node.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, ctrlKey: mods.includes("ctrl"), shiftKey: mods.includes("shift"), altKey: mods.includes("alt") }));
    return { key };
  },
  // The region's rectangle (CSS px) with `pad` around it, inside the window; null = the whole page.
  rect(region, pad = 0) {
    if (!region) return null;
    const node = regionNode(region);
    if (!node) throw new Error(`no region "${region}"`);
    const r = node.getBoundingClientRect();
    const x = Math.max(0, r.left - pad), y = Math.max(0, r.top - pad);
    return [x, y, Math.min(innerWidth, r.right + pad) - x, Math.min(innerHeight, r.bottom + pad) - y];
  },
  // After an engine command from the CLI/MCP: follow a newly opened project, else reload its state.
  async refresh(id, project) {
    if (id.startsWith("motion.")) return { refreshed: id };
    if (id === "agent.new_chat") {
      agent.session=await invoke("agent_session",{project:state.project,set:null});agent.messages=[];agent.permissions=[];agent.forms=[];
      renderLog();renderWatch();renderModels();return {refreshed:id,session:agent.session};
    }
    if (project && project !== state.project) return openProject(project);
    if (!project) return showHome();
    state.data = await invoke("project_state", { path: state.project });
    renderTarget();
    renderBlocks();
    renderStoryboard();
    if (state.view === "animation") await animationRefresh();
    return { refreshed: id };
  },
};
