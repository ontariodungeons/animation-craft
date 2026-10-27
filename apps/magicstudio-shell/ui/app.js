// Magic Studio shell: the project view. Data comes from magicstudio-engine through the
// shell's commands; files open with one click; the studio tab hosts the video target's studio.
const { invoke: nativeInvoke, convertFileSrc } = window.__TAURI__.core;
async function invoke(command,params){
 const action=acFeedback.action;
 const companion=command==='engine_command'?window.uiRegistry?.capture(params.id,{...params.params,_request_id:action?.request_id||params.params?._request_id}):null;
 if(companion)window.uiRegistry?.event({...companion,phase:"start"});
 acFeedback.pending++;await acFeedback.report();
 if(command==='engine_command'&&params){params={...params,params:{...params.params,_origin:action?'UI click':'UI callback',_client:'Studio user (unverified label)',_control:action?.control,_request_id:action?.request_id||params.params?._request_id||companion?.request}};}
 if(command==='project_create'||command==='project_open'){params={...params,request:params?.request??action?.request_id??null,control:params?.control??action?.control??null};}
 acFeedback.report(null,null,{stage:'handler_dispatch',control:action?.control||null,request_id:action?.request_id||null,command:params?.id||command});
 try{
  const result=await nativeInvoke(command,params);acFeedback.last={transport:command,id:params?.id||null,status:'success'};
  if(companion)window.uiRegistry?.event({...companion,phase:'success',result});
  if(result?._ui_engine_revision) acFeedback.pendingRevision=result._ui_engine_revision;
  return result;
 }catch(error){if(companion)window.uiRegistry?.event({...companion,phase:'error',error:String(error)});acFeedback.last={transport:command,id:params?.id||null,status:'error',error:String(error).slice(0,300)};acFeedback.report(null,null,{stage:'handler_error',command:params?.id||command,control:action?.control||null,request_id:action?.request_id||null,error:String(error).slice(0,300)});throw error;}
 finally{acFeedback.pending--;await acFeedback.report();}
}
const $ = (id) => document.getElementById(id);

const ICONS = {
  song: '<svg viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 6h16M4 11h16M4 16h10"/></svg>',
  transcript: '<svg viewBox="0 0 24 24"><path d="M3 12h2M7 8v8M11 5v14M15 9v6M19 11v2"/></svg>',
  animate: '<svg viewBox="0 0 24 24"><path d="M12 3l9 5-9 5-9-5 9-5z"/><path d="M3 13l9 5 9-5"/></svg>',
  chev: '<svg viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5l11 7-11 7z"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><path d="M8 5v14M16 5v14"/></svg>',
};

let state = { project: null, data: null, page: 0, open: "song", view: "project", blocksCollapsed: false };

function updateMagicSelection() {
  const layer = motion?.meta?.layers?.find((part) => part.id === motion.selected);
  const text = layer ? `${layer.label || layer.id} · ${layer.id}` : 'No part selected';
  updateAppFooter();
  for (const id of ['magic-target-image']) {
    const node = $(id);
    if (node) { node.textContent = text; node.title = layer ? `Selected part ${layer.label || layer.id} (${layer.id}) on ${motion.character}` : 'Select a part to target image generation'; }
  }
}

function projectAssetGrid(assets,{active=null,prefix='magic-asset'}={}) {
  return el('div',{class:'magic-assets-grid'},assets.map(asset=>magicAssetTile(asset,{active:asset.id===active,prefix,onclick:()=>openAnimation(asset.id),onremove:async()=>{
    try{
      await designCommand('project.remove_asset',{id:asset.id});
      if(state.view==='animation')await animationRefresh();
      toast(`Removed ${asset.name||asset.id}`);
    }catch(e){toast(String(e));}
  }})));
}

function magicAssetTile(asset, { active = false, candidate = false, prefix='magic-asset', onclick, onremove = null }) {
  const name = asset.name || asset.id;
  const image = /\.(png|jpe?g|webp|gif)$/i.test(asset.path || '')
    ? el('img', { src: convertFileSrc(asset.path), alt: '', draggable: 'false' })
    : el('span', { class: 'magic-asset-placeholder' }, 'PART');
  const remove = onremove ? el('span', {
    class: 'forget asset-forget',
    id: `${prefix}-remove-${asset.id}`,
    role: 'button',
    tabindex: '0',
    'data-commands': 'project.remove_asset',
    'aria-label': `Remove ${name} (${asset.id})`,
    title: `Remove ${name} from project`,
    html: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>',
    onclick: (e) => { e.stopPropagation(); onremove(); },
    onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); onremove(); } }
  }) : null;
  const tile=el('button', { class: `magic-asset-tile${active ? ' active' : ''}${candidate ? ' candidate' : ''}`, id: `${prefix}-${asset.id}`,draggable:'true',
    title: candidate ? `${name} · generated candidate awaiting review` : `${name} (${asset.id}) · drag onto a storyboard page or click to animate`, onclick },
    image, el('span', { class: 'magic-asset-name' }, name),
    candidate ? el('span', { class: 'magic-asset-badge' }, 'Candidate') : el('span', { class: 'magic-asset-id' }, asset.id),
    remove);
  tile.addEventListener('dragstart',event=>{event.dataTransfer.setData('application/x-magicstudio-asset',asset.id);event.dataTransfer.setData('text/plain',asset.id);event.dataTransfer.effectAllowed='copy';});return tile;
}

// ---- Unified footer: workflow stages · what the active tab shows · save and studio state ----
const FOOTER_STAGES = [['song', 'Song'], ['lyrics', 'Lyrics'], ['transcript', 'Transcript'], ['storyboard', 'Storyboard'], ['animate', 'Animate'], ['studio', 'Studio']];

function footerStageState(id) {
  if (id === 'studio') return serverUrl() ? 'ok' : videoProject() ? 'warn' : 'idle';
  const seen = projectView.prerequisite(id).state;
  return seen === 'ready' ? 'ok' : seen ? 'warn' : 'idle';
}

async function footerGo(id) {
  if (id === 'studio') return setView('studio');
  if (state.view !== 'project' && await setView('project') === false) return;
  state.open = SONG_SECTIONS.includes(id) ? 'song' : id; state.blocksCollapsed = false; $('blocks').classList.remove('collapsed'); renderBlocks();
}

function footerContext() {
  if (state.view === 'animation') {
    const meta = motion.meta;
    if (motion.mode === 'composite') return 'Composite · images arranged in time';
    if (!meta) return motion.character ? `${motion.character} · loading…` : 'No object chosen';
    const clips = (meta.tracks || []).filter((t) => !t.standard).length;
    const track = motion.rest ? 'Rest pose' : (meta.tracks.find((t) => t.id === motion.track)?.label || motion.track.replaceAll('-', ' '));
    const part = meta.layers.find((l) => l.id === motion.selected);
    return `${motion.character} · ${meta.layers.length} layers / ${clips} clips · ${track} · Selected: ${part ? part.label || part.id : 'none'}`;
  }
  if (state.view === 'studio') { const project = videoProject(); return project ? `Video project: ${project}` : 'No video project yet'; }
  if (state.view === 'analytics') return 'Analytics · Code complexity, token spend & app status';
  const pages = state.data?.storyboard?.previews?.length || 0;
  const objects = (state.data?.assets?.assets || []).length;
  return [pages ? `Storyboard · Page ${state.page + 1} / ${pages}` : 'Storyboard · not built yet', pages ? state.pageTitles?.[state.page] : null,
    `${objects} object${objects === 1 ? '' : 's'}`].filter(Boolean).join(' · ');
}

// Coalesces the many callers (renders, selection, Theatre messages) into one footer render.
function updateAppFooter() {
  if (updateAppFooter.queued) return;
  updateAppFooter.queued = true;
  queueMicrotask(() => { updateAppFooter.queued = false; renderAppFooter(); });
}

function renderAppFooter() {
  if (!$('app-footer')) return;
  $('footer-stages').replaceChildren(...FOOTER_STAGES.map(([id, label]) => {
    const tone = footerStageState(id);
    return el('button', { class: 'footer-stage', id: `footer-stage-${id}`, title: `${label} · ${tone === 'ok' ? 'ready' : tone === 'warn' ? 'needs work' : 'not started'}`,
      onclick: () => footerGo(id).catch((e) => toast(String(e))) }, window.companionBoxes.dot(tone === 'ok' ? 'ready' : tone === 'warn' ? 'needs-attention' : 'idle', '', `footer-stage-${id}`), label);
  }));
  updateStoryboardSaveControls();
  const context = footerContext();
  $('footer-context').textContent = context; $('footer-context').title = context;
  const motionUnsaved = state.view === 'animation' && typeof magicTheatre !== 'undefined' && (magicTheatre.interactionRevision > magicTheatre.savedRevision || !!magicTheatre.gestureActive);
  const unsaved = motionUnsaved || state.data?.design?.dirty === true || projectView.textDrafts.size > 0;
  const saving = projectView.pending > 0;
  $('footer-save-state').replaceChildren(window.companionBoxes.dot(saving ? 'working' : unsaved ? 'needs-attention' : state.project ? 'ready' : 'idle', '', 'footer-save-state'), saving ? 'Saving…' : unsaved ? 'Unsaved edits' : state.project ? 'Project saved' : 'Not saved');
  $('footer-save-state').title = motionUnsaved ? 'Theatre motion has unsaved keyframes · use Save motion' : unsaved ? 'Storyboard changes need saving' : 'Magic Studio project state; HyperFrames saves composition edits in its own editor';
  const url = serverUrl();
  // A Studio that is off is simply idle: grey, never a red problem.
  $('footer-server').replaceChildren(window.companionBoxes.dot(url ? 'ready' : 'idle', '', 'target'), url ? `Studio · ${url.replace(/^https?:\/\//, '').replace(/\/$/, '')}` : 'Studio off');
}
// Theatre's own listener (theatre.js) updates its revisions first; render after it.
window.addEventListener('message', (event) => { if (event.data?.magicTheatre && event.data.type !== 'values') setTimeout(updateAppFooter); });

function toast(text) {
  const t = $("toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => t.classList.remove("show"), 2400);
}

// Every control in this app is built here.
//
// `el()` is therefore the ONE place a control joins the shared contracts, and
// it inherits BOTH of them:
//
//   * THE BOX -- a stable id, the project and context it belongs to, and the
//     state the app knows about it (idle, working, ready, needs-attention,
//     failed) with its reason, priority, request, job and resolution. Every box
//     exposes the same "scent", so the app light, the companion and the TUI all
//     react to one observation instead of three guesses.
//
//   * THE SLOT -- membership of a rail. A slot declares a stable id, the
//     destinations or roles it accepts, and an explicit priority. Every rail is
//     the same reusable component, so a control can join a rail without
//     knowing anything about how rails work.
//
// A control is in both contracts BY CONSTRUCTION: there is no per-block
// companion logic anywhere and no block has to opt in.
//
// Box ids must be STABLE across renders, so they come from explicit intent
// first (the `box` attribute, then the element's own id) and only fall back
// to a label. Labels change; ids are contracts.
// THE CONFIG. One entry per element id says which component it is, which look
// it takes and how dense it is. el() reads it by id, so a look is chosen in
// the config, never at the call site.
const uiConfig = { version: 0, looks: {}, densities: [], elements: {} };
window.uiConfig = uiConfig;

async function loadComponents() {
  const response = await fetch("components.json");
  if (!response.ok) throw new Error(`components.json ${response.status}`);
  Object.assign(uiConfig, await response.json());
  for (const [id, entry] of Object.entries(uiConfig.elements)) {
    const node = document.getElementById(id);
    if (node) { applyEntry(node, entry, id); node.setAttribute("data-box-id", id); window.companionBoxes?.register(id, { project: state.project, ...entry }); }
  }
  for (const node of document.body.querySelectorAll("*")) window.uiRegistry?.registerElement(node);
  return uiConfig;
}

function applyEntry(node, entry, id = node.id) {
  if (!entry) return node;
  node.dataset.uiConfig = id;
  if (entry.component) node.className = [...new Set(`${node.className || ''} ${entry.component}`.trim().split(/\s+/))].join(' ');
  if (entry.look) node.dataset.look = entry.look;
  if (entry.density) node.dataset.density = entry.density;
  if (entry.component === 'panel' && entry.rail && !node.querySelector('[data-companion-slot]')) {
    const head = node.querySelector('.panel-head');
    if (!head) { node.dataset.contractError = 'Panel has no shared header'; return node; }
    delete node.dataset.contractError;
    node.setAttribute('data-companion-rail', entry.rail);
    const slot = document.createElement('span');
    slot.className = 'panel-rail'; slot.setAttribute('data-companion-slot', entry.rail);
    slot.setAttribute('aria-hidden', 'true'); head.append(slot);
    window.uiRegistry?.registerElement(slot);
  }
  return node;
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  let box = null;
  for (const [k, v] of Object.entries(attrs)) {
    if(v===null||v===undefined)continue;
    if (k === "class") node.className = v;
    else if (k === "html") node.innerHTML = v;
    else if (k === "box") box = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), event=>{const previous=acFeedback.action;acFeedback.action=event.acAction;try{return v(event);}finally{acFeedback.action=previous;}});
    else if (typeof v === "boolean" && k in node) node[k] = v;
    else node.setAttribute(k, v);
  }
  for (const c of children.flat()) if (c != null) node.append(c);
  const project = typeof state === "undefined" ? null : state.project;
  const boxId = box || attrs.id || null;
  const entry = boxId ? uiConfig.elements[boxId] : null;
  if (entry) applyEntry(node, entry, boxId);
  if (boxId) {
    node.setAttribute("data-box-id", boxId);
    if (window.companionBoxes) window.companionBoxes.register(boxId, { project, label: entry?.label || null, look: entry?.look || null, density: entry?.density || null });
  }
  window.uiRegistry?.registerElement(node);
  return node;
}

const kindOf = (name) => (name.split(".").pop() || "file").toUpperCase().slice(0, 4);

function fileButton(chip) {
  return el("button", { class: "file", title: chip.path, "aria-label": `Open ${chip.name}`, onclick: () => openPath(chip.path) },
    el("span", { class: "kind" }, kindOf(chip.name)), el("span", { class: "name" }, chip.name));
}

async function openPath(path) {
  try { await invoke("open_path", { path }); toast(`Opened ${path.split("/").pop()}`); }
  catch (e) { toast(String(e)); }
}

// ---- Blocks -------------------------------------------------------------------------------

const SONG_SECTIONS = ['lyrics', 'transcript'];

// ---- Tabs and the studio webview ----------------------------------------------------------

function placeInk() {
  const active = document.querySelector(".tab.active");
  const ink = document.querySelector(".tab-ink");
  ink.style.setProperty('--ink-w', `${active.offsetWidth - 16}px`);
  ink.style.setProperty('--ink-x', `${active.offsetLeft + 8}px`);
}

async function showStudio() {
  const url = serverUrl();
  $("studio-empty").classList.toggle("show", !url);
  if (!url) return;
  try { await invoke("studio_show", { url, mode: state.view }); }
  catch (e) { toast(String(e)); throw e; }
}

async function setView(view, action) {
  const errors = acFeedback.errors.length;
  if (['design','animation'].includes(view) && !state.data?.design?.progression?.allowed && !state.animateOnly) {
    toast(state.data?.design?.progression?.reason || 'Complete the storyboard details or choose Proceed with this draft.');
    return false;
  }
  if (view === "split") view = "studio";
  if (state.view === "animation" && view !== "animation" && typeof theatreCanLeave === "function" && !theatreCanLeave()) return false;
  if (typeof animationPause === "function") animationPause();
  if (view !== 'animation' && typeof blenderDispose === 'function') blenderDispose();
  state.view = view;
  document.querySelectorAll(".tab").forEach((t) => {
    const on = t.dataset.view === view;
    t.classList.toggle("active", on);
    t.setAttribute("aria-selected", String(on));
  });
  const page = view === "studio" ? "project" : view;
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${page}`));
  placeInk();
  if (view === "animation") { await invoke("studio_hide"); await animationOpen(); }
  else if (view === "design") { await invoke("studio_hide"); await wordsOpen(); }
  else if (view === "skills") { await invoke("studio_hide"); if (typeof renderSkills === "function") renderSkills(); }
  else if (view === "analytics") { await invoke("studio_hide"); if (typeof renderAnalytics === "function") renderAnalytics(); }
  else if (view !== "project" && serverUrl()) { await new Promise(requestAnimationFrame); await showStudio(); }
  else {
    await invoke("studio_hide");
    if (view !== "project") {
      document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-studio"));
      $("studio-empty").classList.add("show");
    }
  }
  updateAppFooter();
  window.magicCompanions?.sync();
  if (action && feedbackSnapshot().rendered && acFeedback.errors.length === errors) {
    await acFeedback.report(null, null, {stage:"ui_navigation_applied",...action,view:state.view});
  }
}

function updateStoryboardAccess() {
  const allowed=state.data?.design?.progression?.allowed===true||state.animateOnly===true;
  for(const button of document.querySelectorAll('.tab[data-view="animation"],.tab[data-view="design"]')) {
    const canOpen=allowed;
    button.disabled=!canOpen;
    button.title=canOpen ? (state.data?.design?.progression?.draft_override?'Draft access enabled; storyboard details still need work':button.textContent.trim()) : 'Complete storyboard details, or choose Proceed with this draft in Storyboard.';
  }
}

// ---- Studio server: red = off (no memory used), green = running --------------------------

function serverUrl() {
  const server = state.data?.status?.video_server || {};
  return server.running === true ? server.url : null;
}

// The video project lives in the engine session (workshop.status / video.init); the UI keeps no copy.
function videoProject() {
  const s = state.data?.status;
  return s?.video_project_path || s?.video_project || null;
}

function renderTarget() {
  const s = state.data?.status;
  if (!s) return;
  const url = serverUrl();
  const project = videoProject();
  $("target-name").textContent = `${s.video_target}${url ? " · " + url.replace(/^https?:\/\//, "").replace(/\/$/, "") : " · off"}`;
  const serverDot = window.companionBoxes.dot(url ? "ready" : "idle", "", "target");
  serverDot.id = "server-dot";
  $("server-dot").replaceWith(serverDot);
  updateAppFooter();
  $("target").title = `${url ? `Studio running at ${url} · click to stop` : "Studio off · click to start"}\n${project ? `Video project: ${project}` : "No video project yet · starting the studio creates it"}`;
}

async function toggleServer() {
  if (!state.project) return;
  const on = !serverUrl();
  const startingDot = window.companionBoxes.dot("working", "pulse", "target");
  startingDot.id = "server-dot";
  $("server-dot").replaceWith(startingDot);
  try {
    // A target without its project gets one from the engine first (video.init reuses an existing one).
    if (on && state.data?.status?.video_target && !state.data.status.video_project_path) {
      await designCommand("video.init", {});
    }
    await designCommand(on ? "video.server_start" : "video.server_stop");
    toast(on ? "Studio started" : "Studio stopped");
  } catch (e) { toast(String(e)); }
  renderTarget();
  if (["studio", "split"].includes(state.view)) (serverUrl() ? showStudio() : setView(state.view));
}

// ---- Startup ------------------------------------------------------------------------------

async function load() {
  await loadComponents();
  if (typeof setupSkillsAdd === "function") setupSkillsAdd();
  const start = await invoke("startup");
  state.identity=start.identity;
  if (start.animate_only) {
    state.animateOnly = true;
    document.body.classList.add("animate-only");
  }
  if (start.character && typeof motion !== "undefined") {
    motion.character = start.character;
    motion.explicitCharacter = true;
  }
  const identity=start.identity||{},port=identity.control_port;
  $('footer-instance').textContent=`Port ${port??'unavailable'} · ${identity.branch||'unknown build'}`;
  $('footer-instance').title=`${identity.build_id||'Unknown build'} · PID ${identity.pid??'?'} · ${identity.executable||''} · Click to copy the CLI command`;
  if (start.new_project) return beginNewProject();
  if (!start.project) return showHome();
  await openProject(start.project, start);
}

// Opens a project (engine `project.open`, recorded in Recent) and shows the Project view.
async function openProject(path, start = {}) {
  try { return await applyOpenedProject(await invoke("project_open", { path }), start); }
  catch (e) { toast(String(e)); throw e; }
}

// Apply an already executed CLI/bridge transition; never dispatch Open while refreshing it.
async function applyOpenedProject(opened, start = {}) {
  if(opened.reused_session){toast(opened.opened_new_instance?"Opened project in a separate window":"Focused the existing project session");return;}
  const path=opened.project;
  if(!path)throw new Error('Project command did not return its canonical location');
  state.data=opened;
  state.projectReceipt=opened;
  if (state.project !== path) { animationReset(); songStop(); transcriptReaderClose(); projectView.textDrafts.clear(); projectView.error=null; state.storyboardScene=null; state.storyboardUndo=null; }
  state.project = path;
  state.lyrics = null;
  loadSongLyrics();
  window.magicCompanions?.sync();
  state.page = 0;
  const startOpen = start.open && start.open !== "agent" ? start.open : "song";
  state.open = SONG_SECTIONS.includes(startOpen) ? "song" : startOpen;
  document.body.classList.remove("at-home");
  $("project-name").textContent = path.split("/").pop().replace(/\.magic$/, "");
  document.title = `${$("project-name").textContent} · Magic Studio`;
  renderTarget();
  renderBlocks();
  renderStoryboard();
  await setView(start.view || "project");
  window.workshopDrawers?.close(false);
  await acFeedback.acknowledge(state.data?._ui_engine_revision, path);
}

document.querySelectorAll(".tab").forEach((t) => t.addEventListener("click", () => setView(t.dataset.view, acFeedback.actions.get(t))));

$("target").addEventListener("click", toggleServer);
$("footer-server").addEventListener("click", toggleServer);
$("footer-instance").addEventListener("click",async()=>{const port=state.identity?.control_port;if(!port)return;try{await navigator.clipboard.writeText(`mc --bridge ${port} exec ui.feedback`);toast(`Copied Studio port ${port}`);}catch{toast(`Studio control port ${port}`);}});
$("studio-start").addEventListener("click", toggleServer);
window.addEventListener("resize", placeInk);
placeInk();
window.addEventListener('DOMContentLoaded',()=>load().catch(e=>{toast(String(e));acFeedback.errors.push({message:String(e).slice(0,300)});acFeedback.report('error');}));
