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

window.magicBridge = {
  state() {
    return {
      companions: window.magicCompanions?.snapshot() || null,
      animation: typeof animationFeedback === "function" ? animationFeedback() : null,
      project: state.project, view: document.body.classList.contains("at-home") ? "home" : state.view, open_block: state.open,
      storyboard_page: state.page + 1, agent_open: agent.open, agent_session: agent.session,
      studio: serverUrl?.() || null, toast: $("toast").classList.contains("show") ? $("toast").textContent : null,
    };
  },
  components(params={}) {
    const root=regionNode(params.region||'#view-project');if(!root)throw new Error('Unknown component region');
    const nodes=[root,...root.querySelectorAll('[id],button,[role],h1,h2,h3,label')].filter(n=>!n.closest('#agent-log,#agent-messages'));
    const limit=Math.min(80,Math.max(1,Number(params.limit)||30));
    const entries=nodes.filter(n=>!params.filter||idOf(n).toLowerCase().includes(String(params.filter).toLowerCase())).slice(0,limit).map((n,i)=>{
      const r=n.getBoundingClientRect(),style=getComputedStyle(n);const input=n.matches('input,textarea,[contenteditable=true]');
      const result={id:n.id||`${root.id||'region'}/${n.tagName.toLowerCase()}-${i}`,tag:n.tagName.toLowerCase(),text:input?null:(n.matches('button,h1,h2,h3,label,[role=tab],#project-name,#sb-empty p,#sb-empty strong')?n.textContent.trim().slice(0,160):null),rect:[r.x,r.y,r.width,r.height],visible:visible(n),enabled:!n.disabled,selected:n.getAttribute('aria-selected')||n.getAttribute('aria-pressed')||n.classList.contains('active'),commands:(n.dataset.commands||'').split(' ').filter(Boolean),mapping:n.dataset.commands?'verified handler annotation':'unknown',style:{}};
      for(const k of ['display','visibility','opacity','color','backgroundColor','fontSize','fontWeight','overflow','position'])result.style[k]=style[k];
      return result;
    });return {ok:true,region:params.region||'#view-project',viewport:[innerWidth,innerHeight],components:entries,truncated:nodes.length>limit,phase:acFeedback.phase,visual_correctness:'requires screenshot review'};
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
  // Every clickable control that is not inside a panel the registry knows:
  // a control with no data-box-id ancestor (or one the registry never saw) is
  // reported as unconfigured instead of being silently ignored.
  unconfigured() {
    const registry = window.companionBoxes;
    const known = (node) => {
      const host = node.closest("[data-box-id]");
      const boxId = host && host.getAttribute("data-box-id");
      return !!(boxId && registry && typeof registry.get === "function" && registry.get(boxId));
    };
    const rows = [];
    const nodes = [...document.querySelectorAll(CLICKABLE)];
    nodes.forEach((node, i) => {
      if (!visible(node) || known(node)) return;
      const tag = node.tagName.toLowerCase();
      const label = (node.title || node.getAttribute("aria-label") || node.textContent || "").trim().slice(0, 80);
      rows.push({ id: node.id || `${tag}-${i + 1}`, tag, label: label || null });
    });
    return rows.slice(0, 64);
  },
  // The generic flag setter: `ui.panel id=<panel> flag=<open|busy|dirty> value=<bool>`.
  // The panel's own handler does the work; a flag it does not accept is refused.
  panel(id, flag, value) {
    const registry = window.companionBoxes;
    const record = registry && typeof registry.get === "function" ? registry.get(id) : null;
    if (!record) throw new Error(`panel ${id} is not registered`);
    const handler = record.on && record.on[flag];
    if (typeof handler !== "function") throw new Error(`panel ${id} does not accept ${flag}`);
    if (typeof registry.flag === "function") registry.flag(id, flag, !!value);
    handler(!!value);
    return { panel: id, flag, value: !!value };
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
  async refresh(id, project, revision, result = {}, request_id = null, params = {}) {
    if(project===state.project)window.magicCompanions?.event({command:id,project,view:state.view,request:request_id||result._request_id||`cli-revision-${revision}`,phase:'success',params,result});
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
