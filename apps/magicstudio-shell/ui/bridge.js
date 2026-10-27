// The page's half of the `--control` bridge (src/bridge.rs): what the CLI and MCP see and click.
// An element's id is its DOM id, else its aria-label, else its visible text; a region is a DOM id
// or a CSS selector.
const CLICKABLE = "button, [role=button], [role=tab], a[href], input, select, textarea, .chip, .object, .lyric";

function visible(node) {
  const r = node.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && getComputedStyle(node).visibility !== "hidden" && !node.closest("[aria-hidden=true], [hidden]");
}

function regionNode(region) {
  if (!region) return document.body;
  return document.getElementById(region) || document.querySelector(region);
}

// A visual inline style: any property set on the element itself. Custom
// properties carry data (geometry), not style.
function visualInlineStyle(node) {
  const style = node.style;
  if (!style) return false;
  const names = typeof style.length === "number" && typeof style.item === "function"
    ? Array.from({ length: style.length }, (_, i) => style.item(i))
    : Object.keys(style).filter(key => !key.startsWith("--"));
  return names.some(name => !name.startsWith("--"));
}

function idOf(node) {
  return node.id || node.getAttribute("aria-label") || (node.innerText || node.value || node.placeholder || "").trim().split("\n")[0].slice(0, 60);
}

function find(target, instance = null) {
  if (/^@[0-9]+$/.test(target) && instance !== window.uiRegistry.instance) throw new Error("Indexed targets require their component instance; inspect again");
  if (!target) throw new Error("which element? pass its id");
  const byId = /^@[0-9]+$/.test(target) ? document.querySelector(`[data-ui-index="${target.slice(1)}"]`) : document.getElementById(target);
  if (byId) return byId;
  const nodes = [...document.querySelectorAll(CLICKABLE)].filter(visible);
  const hit = nodes.find((n) => idOf(n) === target) || nodes.find((n) => idOf(n).toLowerCase().includes(String(target).toLowerCase()));
  if (hit) return hit;
  try { const q = document.querySelector(target); if (q) return q; } catch { /* not a selector */ }
  throw new Error(`no element "${target}"; ui.elements lists them`);
}

window.magicBridge = {
  controls(params = {}) {
    const report = window.uiRegistry.inspect({ ...params, root: regionNode(params.region) });
    return { ...report, component_instance: report.instance };
  },
  attention(params = {}) {
    if (params.project !== state.project) throw new Error("Attention project does not match this Studio");
    window.companionBoxes.attention(params.id, params.clear ? null : params.reason);
    return { ok: true, project: state.project, panels: window.companionBoxes.panels() };
  },
  state() {
    return {
      identity: state.identity||null,
      companions: window.magicCompanions?.snapshot() || null,
      animation: typeof animationFeedback === "function" ? animationFeedback() : null,
      project: state.project, view: document.body.classList.contains("at-home") ? "home" : state.view, open_block: state.open,
      storyboard_page: state.page + 1, drawer: window.workshopDrawers?.active||null, agent_open: agent.open, agent_session: agent.session,
      studio: serverUrl?.() || null, toast: $("toast").classList.contains("show") ? $("toast").textContent : null,
    };
  },
  components(params = {}) {
    const report = this.controls(params);
    return { ...report, components: report.items, viewport: [innerWidth, innerHeight], phase: acFeedback.phase };
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
  // A control is tracked when it, or an ancestor, is a panel the registry
  // knows or has an entry in the config. A control with neither is untracked.
  // A control with a visual inline style is unstyled; geometry set as custom
  // properties is data, not styling, and is never unstyled.
  unconfigured() {
    const registry = window.companionBoxes;
    const entries = (window.uiConfig && window.uiConfig.elements) || {};
    const tracked = (node) => {
      for (let n = node; n; n = n.parentElement) {
        if (n.id && entries[n.id]) return true;
        const host = n.getAttribute && n.getAttribute("data-box-id");
        if (host && registry && typeof registry.get === "function" && registry.get(host)) return true;
      }
      return false;
    };
    const rows = [];
    const nodes = [...document.querySelectorAll(`${CLICKABLE}, [id]`)];
    nodes.forEach((node, i) => {
      const ownState = node.getAttribute('data-box-id') || (entries[node.id] ? node.id : null);
      const reason = node.dataset.contractError ? "missing-panel-header" : (ownState && !registry?.get(ownState)) || !tracked(node) ? "untracked" : visualInlineStyle(node) ? "unstyled" : null;
      if (!reason) return;
      const tag = node.tagName.toLowerCase();
      const label = (node.title || node.getAttribute("aria-label") || node.textContent || "").trim().slice(0, 80);
      const rect = node.getBoundingClientRect(), container = entries[node.id]?.rail || node.closest('[data-companion-rail]')?.getAttribute('data-companion-rail') || null;
      rows.push({ id: node.id || `${tag}-${i + 1}`, index: registry?.registerElement(node)?.index ?? null, tag, label: label || null, reason, container, rect: [rect.x,rect.y,rect.width,rect.height] });
    });
    return rows;
  },
  click(target, instance = null, files = null) {
    const node = find(target, instance);
    if (!visible(node) || node.disabled) throw new Error("Control is hidden or disabled");
    node.scrollIntoView({ block: "nearest" });
    if (files !== null && (!['asset-import','song-import-transcript','song-attach-original'].includes(node.id) || !Array.isArray(files) || !files.length || files.some(p=>typeof p!=='string'||!p.trim()))) throw new Error('files requires an import control and an array of file paths');
    node.uiFiles = files;
    try { node.click(); } finally { delete node.uiFiles; }
    return { clicked: idOf(node) };
  },
  drag(params) {
    const source=find(params.source,params.instance), target=find(params.target,params.instance);
    if(!source.draggable || !visible(source) || !visible(target))throw new Error('Choose a visible draggable asset and a visible storyboard page');
    const x=Number(params.x), y=Number(params.y);
    if(!Number.isFinite(x)||!Number.isFinite(y)||x<0||x>1||y<0||y>1)throw new Error('x and y must be fractions from 0 to 1 within the target');
    target.scrollIntoView({block:'nearest'});
    const rect=target.getBoundingClientRect(), dataTransfer=new DataTransfer();
    source.dispatchEvent(new DragEvent('dragstart',{bubbles:true,dataTransfer}));
    const options={bubbles:true,cancelable:true,dataTransfer,clientX:rect.left+rect.width*x,clientY:rect.top+rect.height*y};
    target.dispatchEvent(new DragEvent('dragover',options));
    target.dispatchEvent(new DragEvent('drop',options));
    source.dispatchEvent(new DragEvent('dragend',{bubbles:true,dataTransfer}));
    return {source:idOf(source),target:idOf(target),drop_dispatched:true};
  },
  set(target, value, instance = null) {
    const node = find(target, instance);
    if (!visible(node) || node.disabled || node.readOnly) throw new Error("Control is not editable");
    if (node.type === 'checkbox' || node.type === 'radio') {
      if (typeof value !== 'boolean') throw new Error("Toggle value must be boolean");
      node.checked = value;
    } else if (node.isContentEditable) node.textContent = String(value);
    else node.value = value;
    node.dispatchEvent(new Event("input", { bubbles: true }));
    node.dispatchEvent(new Event("change", { bubbles: true }));
    return { set: idOf(node) };
  },
  key(key, target, instance = null) {
    const node = target ? find(target, instance) : document.activeElement || document.body;
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
  async refresh(id, project, revision, result = {}, request_id = null, params = {}) {
    if(project===state.project)window.uiRegistry?.event({command:id,project,view:state.view,request:request_id||result._request_id||`cli-revision-${revision}`,phase:'success',params,result});
    try {
    if (id === "agent.new_chat") {
      await agentLoad(); agent.newChat = !result.session;
      await acFeedback.acknowledge(revision,project);
      return {refreshed:id,session:result.session,ack_engine_revision:revision};
    }
    if (project && project !== state.project) {const data=await invoke('project_state',{path:project});await applyOpenedProject({...result,...data,project,_ui_engine_revision:revision});return {refreshed:id,project,ack_engine_revision:revision};}
    if (!project) {await showHome();await acFeedback.acknowledge(revision,null);return {refreshed:id,ack_engine_revision:revision};}
    state.data = await invoke("project_state", { path: state.project });
    if(id==='workshop.build_storyboard'&&result.draft&&result.ok){
      state.data.storyboard=result;state.data.design={placements:[],text_fields:[]};
      toast(result.status||'Draft ready · review before Save');
    }
    renderTarget();
    renderBlocks();
    renderStoryboard();
    if (state.view === "animation") {await animationRefresh();motion.refreshError=null;}
    await acFeedback.acknowledge(revision,project);
    return { refreshed: id, ack_engine_revision:revision };
    }catch(error){
      const message=String(error);
      if(typeof motion!=='undefined')motion.refreshError=message;
      await acFeedback.report(null,null,{stage:'refresh_error',command:id,engine_succeeded:true,error:message});
      return {refreshed:false,command:id,engine_succeeded:true,refresh_error:message,ack_engine_revision:null};
    }
  },
};
