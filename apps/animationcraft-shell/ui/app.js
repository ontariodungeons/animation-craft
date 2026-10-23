// AnimationCraft shell: the project view. Data comes from animationcraft-engine through the
// shell's commands; files open with one click; the studio tab hosts the video target's studio.
const { invoke: nativeInvoke, convertFileSrc } = window.__TAURI__.core;
async function invoke(command,params){
 const action=acFeedback.action;
 acFeedback.pending++;await acFeedback.report();
 if(command==='engine_command'&&params){params={...params,params:{...params.params,_origin:action?'UI click':'UI callback',_client:'Studio user (unverified label)',_control:action?.control,_request_id:action?.request_id}};}
 if(command==='project_create'||command==='project_open'){params={...params,request:params?.request??action?.request_id??null,control:params?.control??action?.control??null};}
 acFeedback.report(null,null,{stage:'handler_dispatch',control:action?.control||null,request_id:action?.request_id||null,command:params?.id||command});
 try{
  const result=await nativeInvoke(command,params);acFeedback.last={transport:command,id:params?.id||null,status:'success'};
  if(result?._ui_engine_revision) acFeedback.pendingRevision=result._ui_engine_revision;
  return result;
 }catch(error){acFeedback.last={transport:command,id:params?.id||null,status:'error',error:String(error).slice(0,300)};acFeedback.report(null,null,{stage:'handler_error',command:params?.id||command,control:action?.control||null,request_id:action?.request_id||null,error:String(error).slice(0,300)});throw error;}
 finally{acFeedback.pending--;await acFeedback.report();}
}
const $ = (id) => document.getElementById(id);

const ICONS = {
  song: '<svg viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 6h16M4 11h16M4 16h10"/></svg>',
  transcript: '<svg viewBox="0 0 24 24"><path d="M3 12h2M7 8v8M11 5v14M15 9v6M19 11v2"/></svg>',
  animate: '<svg viewBox="0 0 24 24"><path d="M12 3l9 5-9 5-9-5 9-5z"/><path d="M3 13l9 5 9-5"/></svg>',
  chev: '<svg viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
};

let state = { project: null, studioUrl: null, data: null, page: 0, open: "song", view: "project", blocksCollapsed: false };

function setupMagicSplitters() {
  const workstation = $('magic-workstation');
  if (!workstation || workstation.dataset.splittersReady) return;
  workstation.dataset.splittersReady = 'true';
  const sizes = { chat: 320, image: 270 };
  for (const key of Object.keys(sizes)) {
    const saved = Number(localStorage.getItem(`magic.${key}-height`));
    if (Number.isFinite(saved) && saved >= 120 && saved <= 700) sizes[key] = saved;
  }
  const apply = () => {
    workstation.style.setProperty('--magic-chat-h', `${sizes.chat}px`);
    workstation.style.setProperty('--magic-image-h', `${sizes.image}px`);
  };
  apply();
  for (const splitter of workstation.querySelectorAll('.magic-splitter')) {
    const key = splitter.dataset.splitter;
    const change = (delta) => {
      sizes[key] = Math.max(key === 'chat' ? 230 : 170, Math.min(700, sizes[key] + delta));
      apply();
      localStorage.setItem(`magic.${key}-height`, String(sizes[key]));
    };
    splitter.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      splitter.setPointerCapture(event.pointerId);
      let last = event.clientY;
      const move = (next) => { change(next.clientY - last); last = next.clientY; };
      const done = () => {
        splitter.removeEventListener('pointermove', move);
        splitter.removeEventListener('pointerup', done);
        splitter.removeEventListener('pointercancel', done);
      };
      splitter.addEventListener('pointermove', move);
      splitter.addEventListener('pointerup', done);
      splitter.addEventListener('pointercancel', done);
    });
    splitter.addEventListener('keydown', (event) => {
      if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
      event.preventDefault();
      change(event.key === 'ArrowDown' ? 12 : -12);
    });
  }
}
setupMagicSplitters();

function updateMagicSelection() {
  const layer = motion?.meta?.layers?.find((part) => part.id === motion.selected);
  const text = layer ? `${layer.label || layer.id} · ${layer.id}` : 'No part selected';
  for (const id of ['magic-target-chat', 'magic-target-image', 'magic-target-parts']) {
    const node = $(id);
    if (node) { node.textContent = text; node.title = layer ? `Selected part ${layer.label || layer.id} (${layer.id}) on ${motion.character}` : 'Select a part to target image generation'; }
  }
}

function renderMagicAssetCatalog() {
  if (state.view === 'animation') return;
  const slot = $('magic-parts-slot');
  if (!slot) return;
  const assets = state.data?.assets?.assets || [];
  const draft = imageDrafts?.get(state.project);
  slot.replaceChildren(el('div', { class: 'group-label' }, 'Project assets'));
  if (!assets.length) { slot.append(el('p', { class: 'why' }, 'Project parts and generated candidates will appear here.')); return; }
  for (const asset of assets) {
    const candidate = draft?.result?.asset_id === asset.id;
    const image = /\.(png|jpe?g|webp|gif)$/i.test(asset.path || '')
      ? el('img', { src: convertFileSrc(asset.path), alt: '', draggable: 'false' })
      : el('span', { class: 'magic-asset-placeholder' }, 'PART');
    const row = el('button', { class: `magic-asset-row${candidate ? ' candidate' : ''}`, id: `magic-asset-${asset.id}`,
      title: candidate ? `${asset.name || asset.id} · generated candidate awaiting review` : `${asset.name || asset.id} · show its parts`,
      onclick: () => openAnimation(asset.id) }, image, el('span', { class: 'magic-asset-name' }, asset.name || asset.id),
      el('span', { class: 'muted' }, candidate ? 'Candidate' : asset.id));
    slot.append(row);
  }
}

function toast(text) {
  const t = $("toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => t.classList.remove("show"), 2400);
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k === "html") node.innerHTML = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), event=>{const previous=acFeedback.action;acFeedback.action=event.acAction;try{return v(event);}finally{acFeedback.action=previous;}});
    else node.setAttribute(k, v);
  }
  for (const c of children.flat()) if (c != null) node.append(c);
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

function block(id, icon, title, meta, done, body, blockState) {
  const open = state.open === id;
  const head = el("button", { class: "block-head", "aria-label":title, "aria-expanded": String(open), title: `${title}${meta ? " · " + meta : ""}`,
      onclick: () => { state.open = open ? null : id; renderBlocks(); } },
    el("span", { class: `dot ${done ? "ok" : "warn"}`, title: done ? "Ready" : "Needs work" }),
    el("span", { class: "ico", html: icon }),
    el("span", { class: "title" }, title),
    el("span", { class: "meta" }, meta || ""),
    el("span", { class: "chev", html: ICONS.chev }));
  return el("section", { class: `block${open ? " open" : ""}`, "data-state": blockState }, head,
    el("div", { class: "block-body" }, el("div", {}, el("div", { class: "block-inner" }, body))));
}

function sceneLines() {
  // "Scene 3 · arm. · 0:06–0:07" -> {time, lyric}
  const doc=state.data?.status?.stages?.find(s=>s.id==='transcript')?.document;
  if(doc?.paragraphs?.length) return doc.paragraphs.map(p=>({time:`${p.start}–${p.end}s`,lyric:p.text||''}));
  return (state.data?.storyboard?.scene_titles || []).map((t) => {
    const parts = t.split(" · ");
    return { lyric: parts.slice(1, -1).join(" · "), time: (parts.at(-1) || "").split("–")[0] };
  });
}

function objectsFrom(files) {
  const groups = new Map();
  for (const f of files) {
    const m = f.path.match(/\/assets\/(characters|backgrounds)\/([^/]+)\//);
    const key = m ? (m[1] === "characters" ? m[2] : "backgrounds") : "loose";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(f);
  }
  return groups;
}

function objectCard(name, layers, stacked) {
  const stack = el("div", { class: "stack" });
  const img = el("img", { src: convertFileSrc(layers[0].path), alt: "", loading: "lazy" });
  stack.append(img);
  // An object shows all its layers assembled, cropped to the drawing (from the engine).
  if (stacked) invoke("object_preview", { project: state.project, name, layers: layers.map((l) => l.path) })
    .then((p) => { img.src = convertFileSrc(p) + `?v=${Date.now()}`; }).catch((e) => toast(String(e)));
  return el("button", { class: "object", title: `${name}: ${layers.map((l) => l.name).join(", ")}`, "aria-label": `${name}, ${layers.length} layers`,
      onclick: () => openAnimation(name) },
    stack, el("span", { class: "label" }, name), el("span", { class: "count" }, `${layers.length} layer${layers.length === 1 ? "" : "s"}`));
}

function renderBlocks() {
  const s = state.data?.status;
  const box = $("blocks");
  box.replaceChildren();
  if (!s) return;
  const receipt=state.projectReceipt;
  if(receipt?.project===state.project && (receipt.problems?.length || receipt.warnings?.length)){
    box.append(el('div',{class:'why',id:'project-intake-problems',role:'status'},
      el('strong',{},receipt.intake_ready===false?'Intake needs attention':'Project notices'),
      el('p',{},receipt.project),
      ...(receipt.problems||[]).map(problem=>el('p',{},String(problem))),
      ...(receipt.warnings||[]).map(warning=>el('p',{},warning.error||warning.effect||warning.code||String(warning)))));
  }
  // Collapse/expand toggle for the blocks rail
  box.append(el("button", {
    class: `icon rail-toggle${state.blocksCollapsed ? "" : " open"}`,
    title: state.blocksCollapsed ? "Expand blocks rail" : "Collapse blocks rail",
    "aria-label": state.blocksCollapsed ? "Expand blocks rail" : "Collapse blocks rail",
    onclick: () => { state.blocksCollapsed = !state.blocksCollapsed; box.classList.toggle("collapsed", state.blocksCollapsed); renderBlocks(); }
  }, el("span", { html: state.blocksCollapsed ? '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 9l6 6 6-6"/><path d="M18 3H6c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM6 13l6-6-6-6zM12 15l-6 6 6 6z"/></svg>' : '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 9l6 6 6-6"/><path d="M18 3H6c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM6 13l6-6-6-6zM12 15l-6 6 6 6z"/></svg>' })));
  const stage = (id) => s.stages.find((x) => x.id === id) || { inputs: [], outputs: [] };
  const song = stage("song"), transcript = stage("transcript"), animate = stage("animate");
  const audio = song.inputs?.[0];
  const liner = (song.inputs || []).slice(1);

  const observed=id=>state.data?.monitoring?.prerequisites?.find(p=>p.id===id);
  const ready=id=>observed(id)?.state==='ready';
  const songState = observed("song")?.state || "unknown";
  box.append(block("song", ICONS.song, "Song", audio?.name, ready("song"),
    [el("div", { class: "why" }, song.why || ""), audio ? fileButton(audio) : null], songState));

  const lines = sceneLines();
  const lyricsState = observed("lyrics")?.state || "unknown";
  box.append(block("lyrics", ICONS.lyrics, "Lyrics & notes", `${lines.filter((l) => l.lyric && l.lyric !== "Instrumental").length} lines`, ready("lyrics"),
    [...lines.map((l) => el("div", { class: "lyric" }, el("span", { class: "t" }, l.time), el("span", {}, l.lyric))),
     el("div", { class: "group-label" }, "Liner & teaching notes"),
     liner.length ? liner.map(fileButton) : el("div", { class: "why" }, "None in this project yet.")], lyricsState));

  const outputs = (transcript.outputs || []).filter((c) => c.exists !== false && c.path !== audio?.path);
  const transcriptState = observed("transcript")?.state || "unknown";
  box.append(block("transcript", ICONS.transcript, "Transcription", `${outputs.length} files`, ready("transcript"), [el('div',{class:'why'},transcript.why||''),...outputs.map(fileButton)], transcriptState));

  const assets=state.data?.assets?.assets||[];
  const cards=assets.map(a=>{
    const picture=/\.(png|jpe?g|webp|gif)$/i.test(a.path||'')?el('img',{src:convertFileSrc(a.path),alt:a.name,draggable:'false'}):el('span',{html:ICONS.animate});
    const card=el('button',{id:`asset-${a.id}`,class:'object',draggable:'true',title:`${a.name} · drag onto a storyboard page or click to animate`,'aria-label':a.name,
      onclick:()=>openAnimation(a.id)},el('div',{class:'stack'},picture),el('span',{class:'label'},a.name));
    card.addEventListener('dragstart',e=>{e.dataTransfer.setData('application/x-animationcraft-asset',a.id);e.dataTransfer.setData('text/plain',a.id);e.dataTransfer.effectAllowed='copy';});return card;
  });
  const animateState = observed("animate")?.state || "unknown";
  const animateBody = assets.length > 0
    ? [el('div',{class:'group-label'},'Objects'),el('div',{class:'objects'},cards)]
    : [el('div',{class:'group-label'},'Objects'),
       el('button',{id:'asset-import-animate',class:'start',onclick:importProjectAsset},'Import artwork'),
       el('p',{class:'why'},'Drag an object onto a storyboard page once imported.')];
  box.append(block('animate',ICONS.animate,'Animate',`${assets.length} objects`,ready("animate"),animateBody, animateState));
  const assetsState = observed("assets")?.state || "unknown";
  const assetFiles = (s.asset_files||[]);
  const assetsBody = assetFiles.length > 0
    ? [el('button',{id:'asset-import',class:'start',onclick:importProjectAsset},'Add asset'),
       el('p',{class:'why'},'Drag a character above onto a storyboard page.'),
       el('details',{},el('summary',{},'Project files'),...assetFiles.map(fileButton))]
    : [el('button',{id:'asset-import',class:'start',onclick:importProjectAsset},'Add asset'),
       el('p',{class:'why'},'Imported artwork and characters appear here. Drag one onto a storyboard page to place it.')];
  box.append(block('assets',ICONS.animate,'Assets',`${assetFiles.length} files`,ready("assets"),assetsBody, assetsState));
  if (typeof imagePromptBlock === 'function') $('magic-image-tools')?.replaceChildren(imagePromptBlock());
  renderMagicAssetCatalog();

}

// ---- Storyboard: title cards ------------------------------------------------------------

function renderStoryboard() {
  const sb = state.data?.storyboard;
  document.body.classList.toggle("movie-document",sb?.template==="backlot");
  const pages = sb?.previews || [];
  const chips = $("sb-chips");
  chips.replaceChildren();
  const cards = $("sb-cards");
  cards.replaceChildren();
  $("sb-empty")?.remove();
  if(sb?.deck){
    cards.append(el('div',{class:'why'},el('strong',{},sb.pdf_fields?'Editable PDF':'Existing storyboard document'),
      el('p',{},sb.pdf_fields?sb.pdf:sb.deck),el('p',{},'Drag prepared project artwork onto a page and Save new version. Text fields can be edited on the page and save when you finish typing. Pictures keep the existing Save new version action.'),
      ...(sb.next_commands||[]).map(command=>el('pre',{},command))));
  }

  for (const id of ["sb-prev", "sb-next", "sb-pdf", "sb-save-version"]) $(id).disabled = !pages.length;
  if (!pages.length) {
    $("sb-page").textContent = "no storyboard yet";
    const imported=state.data?.status?.stages?.find(s=>s.id==='transcript')?.done===true;
    if(imported){
      cards.append(el('div',{id:'sb-empty',class:'sb-empty'},el('strong',{},sb?.scene_packets_available?'Scene map available · editable deck not built':'Transcript available · storyboard not created'),el('p',{},'Your imported lyrics and timing are retained. Review them in Lyrics & notes. Storyboard generation is a separate step.'),el('button',{id:'sb-review-lyrics',class:'start','aria-label':'Review imported lyrics and timing',onclick:()=>{state.blocksCollapsed=false;state.open='lyrics';$('blocks').classList.remove('collapsed');renderBlocks();}},'Review lyrics & timing')));
      return;
    }
    cards.append(el("div", {id:"sb-empty", class:"sb-empty"},
      el("strong", {}, `Start ${$("project-name").textContent}`),
      el("p", {}, "Add a transcript to plan the song’s scenes."),
      el("button", {id:"sb-import-transcript", "data-commands":"workshop.import_transcript", class:"start", onclick:async () => {
        const path = await dialog.open({title:"Import transcript", filters:[{name:"Transcript", extensions:["json"]}]});
        if (!path) return;
        try {
          await motionCommand("workshop.import_transcript", {path});
          state.data = await invoke("project_state", {path:state.project});
          renderBlocks(); renderStoryboard();
        } catch (e) { toast(String(e)); }
      }}, "Add transcript")));
    return;
  }
  state.page = Math.max(0, Math.min(state.page, pages.length - 1));
  const first = (sb.first_scene_page || 2) - 1;
  const nav = sb.page_titles?.length === pages.length ? sb.page_titles.map((title,i)=>[title,i]) : sb.template === "backlot" ? [...Array.from({length:first},(_,i)=>[`Contents ${i+1}`,i]),...(sb.scene_titles||[]).map((t,i)=>[t,first+i])] : [["Lyrics", 0], ["Cast & sets", 1],
    ...(sb.scene_titles || []).map((t, i) => [t.split(" · ").slice(0, 2).join(" · "), first + i]), ["Brief", pages.length - 1]];
  const isBacklot = sb.template === "backlot";
  const chapterCount = isBacklot ? first : 3;
  for (const [i, [label, page]] of nav.entries()) {
    const active = page === state.page;
    const isChapter = isBacklot ? i < first : i < 2 || i === nav.length - 1;
    chips.append(el("button", { class: `chip${active ? " active" : ""}${isChapter ? " chapter" : " scene"}`, role: "tab", "aria-selected": String(active), title: label,
      onclick: () => showPage(page) }, label));
  }
  chips.querySelector(".active")?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  $("sb-page").textContent = `${state.page + 1} / ${pages.length}`;

  const wrap=cards;
  const key=JSON.stringify([state.project,pages,sb.layout]);
  {
    wrap.dataset.document=key;wrap.replaceChildren();wrap.classList.add("sb-page-wrap","document-pages");
    const size=sb.layout?.page_size||[595,842];
    pages.forEach((path,i)=>{
      const page=el("div",{id:`sb-sheet-${i}`,class:"sb-sheet",style:`aspect-ratio:${size[0]}/${size[1]}`});
      page.append(el("img",{src:convertFileSrc(path),alt:`Storyboard page ${i+1}`,loading:"lazy"}));
      for(const [j,link] of (sb.layout?.links||[]).entries())if(link.page===i+1){
        const [x,y,r,b]=link.rect;
        page.append(el("button",{id:`sb-link-${i}-${j}`,class:"sb-link",title:`Go to page ${link.to_page}`,"aria-label":`Go to page ${link.to_page}`,style:`left:${x/size[0]*100}%;top:${y/size[1]*100}%;width:${(r-x)/size[0]*100}%;height:${(b-y)/size[1]*100}%`,onclick:()=>showPage(link.to_page-1)}));
      }
      page.addEventListener('dragover',e=>{e.preventDefault();page.classList.add('drop-target');e.dataTransfer.dropEffect='copy';});
      page.addEventListener('dragleave',()=>page.classList.remove('drop-target'));
      page.addEventListener('drop',async e=>{
        e.preventDefault();page.classList.remove('drop-target');
        const id=e.dataTransfer.getData('application/x-animationcraft-asset');if(!id)return;
        const r=page.getBoundingClientRect();
        try{await designCommand('storyboard.place_asset',{asset:id,page:i+1,x:(e.clientX-r.left)/r.width*size[0]-80,y:(e.clientY-r.top)/r.height*size[1]-80,width:160});}catch(error){toast(String(error));}
      });
      wrap.append(page);
    });
    wrap.onscroll=()=>{let current=0;const top=wrap.getBoundingClientRect().top;[...wrap.children].forEach((p,i)=>{if(p.getBoundingClientRect().top<=top+wrap.clientHeight*.35)current=i;});state.page=current;$("sb-page").textContent=`${current+1} / ${pages.length}`;};
  }

  renderStoryboardText();
  renderDesignPlacements();
  $(`sb-sheet-${state.page}`)?.scrollIntoView({block:"start",behavior:"instant"});
}

function showPage(n) {
  const total = state.data?.storyboard?.previews?.length || 0;
  state.page = Math.max(0, Math.min(total - 1, n));
  renderStoryboard();
}

// ---- Tabs and the studio webview ----------------------------------------------------------

function placeInk() {
  const active = document.querySelector(".tab.active");
  const ink = document.querySelector(".tab-ink");
  ink.style.width = `${active.offsetWidth - 16}px`;
  ink.style.transform = `translateX(${active.offsetLeft + 8}px)`;
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
  if (view === "split") view = "studio";
  if (typeof animationPause === "function") animationPause();
  state.view = view;
  if (view !== 'animation') renderMagicAssetCatalog();
  document.querySelectorAll(".tab").forEach((t) => {
    const on = t.dataset.view === view;
    t.classList.toggle("active", on);
    t.setAttribute("aria-selected", String(on));
  });
  const page = view === "studio" ? "project" : view;
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${page}`));
  placeInk();
  if (view === "animation") { await invoke("studio_hide"); await animationOpen(); }
  else if (view !== "project" && serverUrl()) { await new Promise(requestAnimationFrame); await showStudio(); }
  else {
    await invoke("studio_hide");
    if (view !== "project") {
      document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-studio"));
      $("studio-empty").classList.add("show");
    }
  }
  if (action && feedbackSnapshot().rendered && acFeedback.errors.length === errors) {
    await acFeedback.report(null, null, {stage:"ui_navigation_applied",...action,view:state.view});
  }
}

// ---- Studio server: red = off (no memory used), green = running --------------------------

function serverUrl() {
  const server = state.data?.status?.video_server || {};
  return state.studioUrl || (server.running ? server.url : null);
}

function renderTarget() {
  const s = state.data?.status;
  if (!s) return;
  const url = serverUrl();
  $("target-name").textContent = `${s.video_target}${url ? " · " + url.replace(/^https?:\/\//, "").replace(/\/$/, "") : " · off"}`;
  $("server-dot").className = `dot ${url ? "ok" : "off"}`;
  $("target").title = url ? `Studio running at ${url} · click to stop` : "Studio off · click to start";
}

async function toggleServer() {
  if (!state.project) return;
  const on = !serverUrl();
  $("server-dot").className = "dot busy";
  try {
    state.data = await invoke("studio_server", { path: state.project, on });
    if (!on) state.studioUrl = null;
    toast(on ? "Studio started" : "Studio stopped");
  } catch (e) { toast(String(e)); }
  renderTarget();
  if (["studio", "split"].includes(state.view)) (serverUrl() ? showStudio() : setView(state.view));
}

// ---- Startup ------------------------------------------------------------------------------

async function load() {
  const start = await invoke("startup");
  state.studioUrl = start.studio || null;
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
  if(opened.reused_session){toast("Focused the existing project session");return;}
  const path=opened.project;
  if(!path)throw new Error('Project command did not return its canonical location');
  state.data=opened;
  state.projectReceipt=opened;
  if (state.project !== path) animationReset();
  state.project = path;
  state.page = 0;
  state.open = start.open && start.open !== "agent" ? start.open : "song";
  document.body.classList.remove("at-home");
  $("project-name").textContent = path.split("/").pop().replace(/\.acproj$/, "");
  document.title = `${$("project-name").textContent} · Magic Studio`;
  renderTarget();
  renderBlocks();
  renderStoryboard();
  await setView(start.view || "project");
  agentToggle(true);
  await acFeedback.acknowledge(state.data?._ui_engine_revision, path);
}

document.querySelectorAll(".tab").forEach((t) => t.addEventListener("click", () => setView(t.dataset.view, acFeedback.actions.get(t))));

// The frame between the project view and the studio: drag to move, double-click for half,
// buttons collapse either side (studio only / project only).
(() => {
  const handle = $("frame-handle");
  if (!handle) return; // The current layout has no legacy splitter; continue startup.
  let total = 0, frame = null;
  handle.addEventListener("pointerdown", async (e) => {
    if (e.target.closest("button")) return;
    handle.setPointerCapture(e.pointerId);
    handle.classList.add("dragging");
    total = await invoke("window_height");
  });
  handle.addEventListener("pointermove", (e) => {
    if (!handle.classList.contains("dragging") || !total) return;
    const ratio = e.clientY / total;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => invoke("set_split", { ratio }));
  });
  handle.addEventListener("pointerup", () => handle.classList.remove("dragging"));
  handle.addEventListener("dblclick", () => invoke("set_split", { ratio: 0.5 }));
  $("collapse-top").addEventListener("click", () => setView("studio"));
  $("collapse-bottom").addEventListener("click", () => setView("project"));
})();
$("target").addEventListener("click", toggleServer);
$("studio-start").addEventListener("click", toggleServer);
$("sb-prev").addEventListener("click", () => showPage(state.page - 1));
$("sb-next").addEventListener("click", () => showPage(state.page + 1));
$("sb-pdf").addEventListener("click", () => state.data?.storyboard?.pdf && openPath(state.data.storyboard.pdf));
window.addEventListener("keydown", (e) => {
  if (state.view !== "project" || e.target.closest("input, textarea")) return;
  if (e.key === "ArrowRight") showPage(state.page + 1);
  if (e.key === "ArrowLeft") showPage(state.page - 1);
});
window.addEventListener("resize", placeInk);
placeInk();
load().catch(e=>{toast(String(e));acFeedback.errors.push({message:String(e).slice(0,300)});acFeedback.report('error');});

$("project-details").addEventListener("click",()=>{document.body.classList.toggle("project-details-open");$("project-details").setAttribute("aria-pressed",String(document.body.classList.contains("project-details-open")));});

function renderStoryboardText(){
 document.querySelectorAll('.sb-text').forEach(node=>node.remove());
 const size=state.data?.storyboard?.page_size||[960,540];
 for(const field of state.data?.design?.text_fields||[]){
  const page=$(`sb-sheet-${field.page-1}`);if(!page)continue;
  const [x,y,w,h]=field.rect;
  const node=el('textarea',{id:`sb-text-${field.page}-${field.id}`,class:'sb-text',
   title:`Edit ${field.field.replaceAll('_',' ')} · saved when you finish typing`,
   'aria-label':`Scene ${field.scene||'page '+field.page} ${field.field.replaceAll('_',' ')}`,
   'data-commands':'storyboard.edit',spellcheck:'true',
   style:`left:${x/size[0]*100}%;top:${y/size[1]*100}%;width:${w/size[0]*100}%;height:${(field.name?h:Math.max(h,50))/size[1]*100}%;font-size:${(field.name?(field.font_size||12):(field.field==='lyrics'?27:21))/size[0]*100}cqw;${field.name?`font-family:Helvetica,Arial,sans-serif;padding:${2/size[0]*100}cqw;box-sizing:border-box;--field-fill:${field.fill};--field-ink:${field.text_color}`:''}`});
  node.value=node.defaultValue=field.value;
  node.addEventListener('change',async()=>{
   if(node.value===node.defaultValue)return;
   node.disabled=true;
   try{await designCommand('storyboard.edit',{page:field.page,field:field.field,value:node.value});toast('Storyboard text saved');}
   catch(error){node.disabled=false;toast(String(error));}
  });
  page.append(node);
 }
}

async function designCommand(id,params={}){
 const result=await motionCommand(id,params);
 state.data=await invoke('project_state',{path:state.project});renderBlocks();renderStoryboard();
 if(result?._ui_engine_revision)await acFeedback.acknowledge(result._ui_engine_revision,state.project);
 return result;
}
async function importProjectAsset(){
 const path=await dialog.open({title:'Add artwork',filters:[{name:'Images',extensions:['png','jpg','jpeg','webp']}]});if(!path)return;
 const name=path.split('/').pop().replace(/\.[^.]+$/,'');
 try{await designCommand('project.add_asset',{path,kind:'image',name,character:name,load:false});state.open='animate';renderBlocks();toast('Asset added · drag it onto a scene');}catch(e){toast(String(e));}
}
let selectedPlacement=null;
function renderDesignPlacements(){
 const entries=state.data?.design?.placements||[];const size=state.data?.storyboard?.page_size||[960,540];
 document.querySelectorAll('.design-placement,.design-remove').forEach(n=>n.remove());
 $('sb-save-version').hidden=!(state.data?.storyboard?.pdf_fields||state.data?.storyboard?.deck);
 if(!entries.some(p=>p.id===selectedPlacement))selectedPlacement=entries.at(-1)?.id||null;

 for(const item of entries){
  const page=$(`sb-sheet-${item.page-1}`);if(!page)continue;
  const [x,y,w,h]=item.rect;
  const node=el('div',{id:item.id,class:'design-placement',title:'Drag to move · grab an edge to scale',style:`left:${100*x/size[0]}%;top:${100*y/size[1]}%;width:${100*w/size[0]}%;height:${100*h/size[1]}%`});
  node.append(el('img',{src:convertFileSrc(item.path),alt:'Placed design asset',draggable:'false'}));
  for(const [hx,hy] of [[-1,-1],[0,-1],[1,-1],[-1,0],[1,0],[-1,1],[0,1],[1,1]]){
   const handle=el('button',{class:'design-handle',title:'Drag to scale', 'aria-label':'Scale placed image',style:`left:${(hx+1)*50}%;top:${(hy+1)*50}%;cursor:${hx===0?'ns':hy===0?'ew':hx===hy?'nwse':'nesw'}-resize`});
   handle.addEventListener('pointerdown',e=>{
    e.preventDefault();e.stopPropagation();selectedPlacement=item.id;handle.setPointerCapture(e.pointerId);
    const r=page.getBoundingClientRect(),start=[e.clientX,e.clientY];let rect=[x,y,w,h];
    handle.onpointermove=event=>{
     const dx=(event.clientX-start[0])*size[0]/r.width,dy=(event.clientY-start[1])*size[1]/r.height;
     const factor=hx===0?1+hy*dy/h:hy===0?1+hx*dx/w:1+(hx*dx/w+hy*dy/h)/2;
     const nw=Math.max(20,Math.min(size[0],w*factor)),nh=nw*h/w;
     rect=[hx<0?x+w-nw:hx===0?x+(w-nw)/2:x,hy<0?y+h-nh:hy===0?y+(h-nh)/2:y,nw,nh];
     node.style.left=`${rect[0]/size[0]*100}%`;node.style.top=`${rect[1]/size[1]*100}%`;node.style.width=`${nw/size[0]*100}%`;node.style.height=`${nh/size[1]*100}%`;
    };
    handle.onpointerup=async()=>{handle.onpointermove=null;handle.onpointerup=null;try{await designCommand('storyboard.place_asset',{asset:item.asset,page:item.page,placement:item.id,x:rect[0],y:rect[1],width:rect[2]});}catch(error){toast(String(error));}};
   });node.append(handle);
  }
  node.addEventListener('pointerdown',e=>{
   selectedPlacement=item.id;
   e.preventDefault();node.setPointerCapture(e.pointerId);const r=page.getBoundingClientRect(),start=[e.clientX,e.clientY];let nx=x,ny=y;
   node.onpointermove=event=>{nx=x+(event.clientX-start[0])*size[0]/r.width;ny=y+(event.clientY-start[1])*size[1]/r.height;node.style.left=`${nx/size[0]*100}%`;node.style.top=`${ny/size[1]*100}%`;};
   node.onpointerup=async()=>{node.onpointermove=null;node.onpointerup=null;try{await designCommand('storyboard.place_asset',{asset:item.asset,page:item.page,x:nx,y:ny,width:w,placement:item.id});}catch(error){toast(String(error));}};
  });page.append(node,el('button',{class:'design-remove',title:'Remove this placement','aria-label':'Remove this placement',style:`left:${100*(x+w)/size[0]}%;top:${100*y/size[1]}%`,onclick:()=>designCommand('storyboard.remove_placement',{placement:item.id}).catch(e=>toast(String(e)))},'×'));
 }
}
$('sb-save-version').addEventListener('click',async()=>{const button=$('sb-save-version');button.disabled=true;button.setAttribute('aria-busy','true');try{await designCommand('storyboard.save_version',state.data?.storyboard?.draft?{draft:state.data.storyboard.manifest}:{});toast('Saved a new editable storyboard version');}catch(e){toast(String(e));}finally{button.disabled=false;button.removeAttribute('aria-busy');}});
