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

// The Generate card folds to its header (folded by default so Parts & assets gets the room).
function setupGenerateToggle() {
  const toggle = $('magic-image-toggle');
  if (!toggle) return;
  const apply = (open) => {
    $('magic-workstation').classList.toggle('generate-collapsed', !open);
    toggle.setAttribute('aria-expanded', String(open));
    localStorage.setItem('magic.generate-open', String(open));
  };
  apply(localStorage.getItem('magic.generate-open') === 'true');
  toggle.addEventListener('click', () => apply(toggle.getAttribute('aria-expanded') !== 'true'));
}
setupGenerateToggle();

function updateMagicSelection() {
  const layer = motion?.meta?.layers?.find((part) => part.id === motion.selected);
  const text = layer ? `${layer.label || layer.id} · ${layer.id}` : 'No part selected';
  updateAppFooter();
  for (const id of ['magic-target-image', 'magic-target-parts']) {
    const node = $(id);
    if (node) { node.textContent = text; node.title = layer ? `Selected part ${layer.label || layer.id} (${layer.id}) on ${motion.character}` : 'Select a part to target image generation'; }
  }
}

function renderMagicAssetCatalog() {
  // Animation and Design fill this pane themselves (parts; effects).
  if (state.view === 'animation' || state.view === 'design') return;
  const slot = $('magic-parts-slot');
  if (!slot) return;
  const assets = state.data?.assets?.assets || [];
  const draft = imageDrafts?.get(state.project);
  slot.replaceChildren(el('div', { class: 'group-label' }, `Project assets (${assets.length})`));
  if (!assets.length) { slot.append(el('p', { class: 'why' }, 'Project parts and generated candidates will appear here.')); return; }
  slot.append(el('div', { class: 'magic-assets-grid' }, assets.map((asset) =>
    magicAssetTile(asset, { candidate: draft?.result?.asset_id === asset.id, onclick: () => openAnimation(asset.id) }))));
}

// One compact tile per project asset: the Project/Studio catalog and the Animation Assets tab.
function magicAssetTile(asset, { active = false, candidate = false, onclick }) {
  const name = asset.name || asset.id;
  const image = /\.(png|jpe?g|webp|gif)$/i.test(asset.path || '')
    ? el('img', { src: convertFileSrc(asset.path), alt: '', draggable: 'false' })
    : el('span', { class: 'magic-asset-placeholder' }, 'PART');
  return el('button', { class: `magic-asset-tile${active ? ' active' : ''}${candidate ? ' candidate' : ''}`, id: `magic-asset-${asset.id}`,
    title: candidate ? `${name} · generated candidate awaiting review` : `${name} (${asset.id}) · show its parts`, onclick },
    image, el('span', { class: 'magic-asset-name' }, name),
    candidate ? el('span', { class: 'magic-asset-badge' }, 'Candidate') : el('span', { class: 'magic-asset-id' }, asset.id));
}

// ---- Unified footer: workflow stages · what the active tab shows · save and studio state ----
const FOOTER_STAGES = [['song', 'Song'], ['lyrics', 'Lyrics'], ['transcript', 'Transcript'], ['storyboard', 'Storyboard'], ['animate', 'Animate'], ['studio', 'Studio']];

function footerStageState(id) {
  if (id === 'storyboard') return state.data?.storyboard?.previews?.length ? 'ok' : 'warn';
  if (id === 'studio') return serverUrl() ? 'ok' : videoProject() ? 'warn' : 'idle';
  const seen = state.data?.monitoring?.prerequisites?.find((p) => p.id === id)?.state;
  return seen === 'ready' ? 'ok' : seen ? 'warn' : 'idle';
}

async function footerGo(id) {
  if (id === 'studio') return setView('studio');
  if (state.view !== 'project' && await setView('project') === false) return;
  if (id === 'storyboard') return;
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
  if (!$('app-footer') || !state.data) return;
  $('footer-stages').replaceChildren(...FOOTER_STAGES.map(([id, label]) => {
    const tone = footerStageState(id);
    return el('button', { class: 'footer-stage', id: `footer-stage-${id}`, title: `${label} · ${tone === 'ok' ? 'ready' : tone === 'warn' ? 'needs work' : 'not started'}`,
      onclick: () => footerGo(id).catch((e) => toast(String(e))) }, window.companionBoxes.dot(tone === 'ok' ? 'ready' : tone === 'warn' ? 'needs-attention' : 'idle', '', `footer-stage-${id}`), label);
  }));
  const context = footerContext();
  $('footer-context').textContent = context; $('footer-context').title = context;
  const unsaved = state.view === 'animation' && typeof magicTheatre !== 'undefined' && (magicTheatre.interactionRevision > magicTheatre.savedRevision || !!magicTheatre.gestureActive);
  $('footer-save-state').replaceChildren(window.companionBoxes.dot(unsaved ? 'needs-attention' : 'ready', '', 'footer-save-state'), unsaved ? 'Unsaved edits' : 'Saved');
  $('footer-save-state').title = unsaved ? 'Theatre motion has unsaved keyframes · use Save motion' : 'No unsaved motion';
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
    if (node) { applyEntry(node, entry); node.setAttribute("data-box-id", id); window.companionBoxes?.register(id, { project: state.project, ...entry }); }
  }
  for (const node of document.body.querySelectorAll("*")) window.uiRegistry?.registerElement(node);
  return uiConfig;
}

function applyEntry(node, entry) {
  if (!entry) return node;
  if (entry.component) node.className = [...new Set(`${node.className || ''} ${entry.component}`.trim().split(/\s+/))].join(' ');
  if (entry.look) node.dataset.look = entry.look;
  if (entry.density) node.dataset.density = entry.density;
  return node;
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  let box = null;
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k === "html") node.innerHTML = v;
    else if (k === "box") box = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), event=>{const previous=acFeedback.action;acFeedback.action=event.acAction;try{return v(event);}finally{acFeedback.action=previous;}});
    else node.setAttribute(k, v);
  }
  for (const c of children.flat()) if (c != null) node.append(c);
  const project = typeof state === "undefined" ? null : state.project;
  const boxId = box || attrs.id || null;
  const entry = boxId ? uiConfig.elements[boxId] : null;
  if (entry) applyEntry(node, entry);
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

function songSection(id, title, meta, done, sectionState, body) {
  const head = window.companionBoxes.dot(done ? "ready" : "needs-attention", "", `song-${id}`);
  head.title = done ? "Ready" : "Needs work";
  return el("div", { class: "song-section", id: `song-${id}`, "data-section": id, "data-state": sectionState },
    el("div", { class: "panel-head" },
      head,
      el("span", { class: "title" }, title), el("span", { class: "meta" }, meta || "")),
    ...body);
}

function transcriptSummary(doc, ok) {
  const words = doc?.words || [];
  if (!words.length) return "No timed words yet";
  const end = doc.recording_duration_sec ?? words.at(-1).end;
  return `${words.length} words · ${clockTime(end)} · ${ok ? "timing ok" : "timing needs review"}`;
}

function clockTime(seconds) {
  const s = Math.max(0, Math.floor(seconds || 0));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

// ---- Song playback: one audio element per project, shared by the Song header and the reader ----
const songPlayer = { audio: null, path: null };

function songAudio(path) {
  if (songPlayer.path !== path) {
    songPlayer.audio?.pause();
    songPlayer.audio = new Audio(convertFileSrc(path));
    songPlayer.path = path;
    for (const type of ["play", "pause", "ended"]) songPlayer.audio.addEventListener(type, renderSongPlay);
    songPlayer.audio.addEventListener("timeupdate", readerFollow);
  }
  return songPlayer.audio;
}

function songStop() { songPlayer.audio?.pause(); songPlayer.audio = null; songPlayer.path = null; }

function songPlayButton(path) {
  const button = el("button", { class: "icon song-play", id: "song-play", "data-path": path,
    onclick: (e) => {
      e.stopPropagation(); // play/pause never opens or closes the Song section
      const audio = songAudio(path);
      if (audio.paused) audio.play().catch((error) => toast(`Cannot play the song: ${error.message || error}`));
      else audio.pause();
    } });
  renderSongPlay(button);
  return button;
}

function renderSongPlay(button = $("song-play")) {
  if (!button?.dataset) return;
  const playing = songPlayer.path === button.dataset.path && songPlayer.audio && !songPlayer.audio.paused;
  const label = playing ? "Pause song" : "Play song";
  button.setAttribute("aria-label", label); button.title = label; button.setAttribute("aria-pressed", String(!!playing));
  button.innerHTML = playing ? ICONS.pause : ICONS.play;
}

// ---- Lyrics: the artist's sheet when the project has one, otherwise the transcript as heard ----
// One model feeds both the Song sheet and the reader, so they always show the same words.
function lyricModel(lyrics, doc, asHeard = false) {
  if (!asHeard && lyrics?.source === "canonical") return { canonical: true, sections: (lyrics.sections || []).map((s) => ({ label: s.label,
    lines: s.lines.map((l) => ({ text: l.text, start: l.start ?? null, words: l.words || [] })) })) };
  const words = (doc?.words || []).filter((w) => Number.isFinite(w.start) && Number.isFinite(w.end));
  const lines = [];
  let line = [];
  words.forEach((w, i) => {
    if (line.length && (w.start - words[i - 1].end >= 0.6 || line.length >= 10)) { lines.push(line); line = []; }
    line.push(w);
  });
  if (line.length) lines.push(line);
  return { canonical: false, sections: [{ label: null, lines: lines.map((l) => ({ text: l.map((w) => w.text).join(" "), start: l[0].start, words: l })) }] };
}

async function loadSongLyrics() {
  const project = state.project;
  try {
    const lyrics = await invoke("engine_command", { id: "song.lyrics", params: {} });
    if (state.project === project) { state.lyrics = lyrics; renderBlocks(); }
  } catch (error) { if (state.project === project) state.lyrics = null; }
}

async function attachOriginal() {
  const path = await dialog.open({ title: "Attach the artist's original lyrics or liner notes" });
  if (!path) return;
  try {
    await designCommand("workshop.add", { kind: "liner_notes", paths: [path] });
    await loadSongLyrics();
    toast("Attached to liner notes; the original file is unchanged");
  } catch (error) { toast(String(error)); }
}

function lyricSheet(model) {
  return el("div", { class: "lyric-sheet" }, ...model.sections.map((section) => el("div", { class: "lyric-stanza" },
    section.label ? el("div", { class: "stanza-label" }, section.label) : null,
    ...section.lines.map((line) => el("div", { class: "lyric-line" }, el("span", { class: "lyric-text" }, line.text),
      line.start != null ? el("span", { class: "cue", title: "Sung at" }, clockTime(line.start)) : null)))));
}

function lyricsBody(lyrics, doc, liner, songPath) {
  const model = lyricModel(lyrics, doc);
  const original = lyrics?.original;
  const name = (path) => String(path).split("/").pop();
  return [
    el("p", { class: "lyric-source" }, model.canonical
      ? `${lyrics.artist ? `${lyrics.artist} · ` : ""}the artist's lyric sheet${lyrics.timing ? " · timing from the recording" : ""}`
      : "From the transcript, as heard and not corrected · no artist lyric sheet attached"),
    model.canonical && lyrics.artist_note ? el("blockquote", { class: "artist-note" }, el("p", {}, lyrics.artist_note), el("cite", {}, lyrics.artist || "The artist")) : null,
    el("div", { class: "lyric-actions" },
      original?.exists
        ? el("button", { id: "song-open-original", class: "ghost", title: original.path, onclick: () => openPath(original.path) }, `Open artist's original · ${name(original.path)}`)
        : [el("span", { class: "not-linked" }, original ? `Original not found: ${name(original.path)}` : "Original not linked"),
           el("button", { id: "song-attach-original", class: "ghost", onclick: attachOriginal }, "Attach original…")],
      model.sections.some((s) => s.lines.length) ? el("button", { id: "song-read-along", class: "ghost", onclick: () => lyricReaderOpen(lyrics, doc, songPath) }, "Read along") : null),
    lyricSheet(model),
    el("div", { class: "group-label" }, "Liner notes"),
    liner.length ? liner.map(fileButton) : el("div", { class: "why" }, "None attached yet."),
  ];
}

// ---- Reader: the same lyric model in the right workstation; read only, follows the song ----
const lyricReader = { words: [] };

function lyricReaderOpen(lyrics, doc, songPath, asHeard = false) {
  if (typeof agentToggle === "function" && !document.body.classList.contains("agent-docked")) agentToggle(true);
  const model = lyricModel(lyrics, doc, asHeard);
  const seek = (t) => { if (songPath && t != null) songAudio(songPath).currentTime = t; };
  const words = [];
  const body = el("div", { class: "reader-body" }, ...model.sections.map((section) => el("div", { class: "lyric-stanza" },
    section.label ? el("div", { class: "stanza-label" }, section.label) : null,
    ...section.lines.map((line) => el("p", { class: "reader-line" },
      line.start != null ? el("button", { class: "reader-time", title: songPath ? "Play from here" : "", "aria-label": `Seek to ${clockTime(line.start)}`, onclick: () => seek(line.start) }, clockTime(line.start)) : null,
      ...(line.words.length
        ? line.words.map((w) => { words.push(w); return el("span", { class: "reader-word", "data-index": String(words.length - 1), title: `${(+w.start).toFixed(2)}–${(+w.end).toFixed(2)} s` }, `${w.text} `); })
        : [el("span", {}, line.text)]))))));
  const title = model.canonical ? "Lyrics" : "Transcript";
  const pane = el("section", { class: "magic-pane magic-reader", id: "magic-reader", "aria-label": `${title}, read only` },
    el("div", { class: "panel-head pane" }, el("span", {}, title),
      el("span", { class: "magic-target" }, model.canonical ? "artist's sheet · read only" : "as heard · read only"),
      el("button", { class: "ghost", id: "magic-reader-close", "aria-label": `Close the ${title.toLowerCase()}`, onclick: transcriptReaderClose }, "Close")),
    body);
  $("magic-reader")?.remove();
  $("magic-workstation")?.append(pane);
  lyricReader.words = words;
  readerFollow();
}

function transcriptReaderClose() { $("magic-reader")?.remove(); lyricReader.words = []; }

function readerFollow() {
  const pane = $("magic-reader");
  if (!pane || !songPlayer.audio) return;
  const t = songPlayer.audio.currentTime;
  const index = lyricReader.words.findIndex((w) => t >= w.start && t < w.end);
  pane.querySelector(".reader-word.now")?.classList.remove("now");
  if (index >= 0) pane.querySelector(`.reader-word[data-index="${index}"]`)?.classList.add("now");
}

// A block is a shared UI box: it has a stable id, and its header row is a
// slot on the rail so a companion can enter the block. The header is the
// stable surface -- the body expands and collapses, so nothing there can
// hold a position.
//
// The body is wrapped, never rewritten: the same DOM arrives, and a block
// that does not care about companions inherits the contract for free.
function block(id, icon, title, meta, done, body, blockState, action) {
  const open = state.open === id;
  const dot = window.companionBoxes.dot(done ? "ready" : "needs-attention", "", id);
  dot.title = done ? "Ready" : "Needs work";
  const head = el("button", { class: "panel-head", id: `block-${id}`, "aria-label":title, "aria-expanded": String(open), title: `${title}${meta ? ` · ${meta}` : ""}`,
      onclick: () => { state.open = open ? null : id; renderBlocks(); } },
    dot,
    el("span", { class: "ico", html: icon }),
    el("span", { class: "title" }, title),
    el("span", { class: "meta" }, meta || ""),
    el("span", { class: "chev", html: ICONS.chev }));
  // The header row carries the slot. It sits on the block's own edge, outside
  // the clickable head button, so a companion never covers a control.
  const slot = el("span", { class: "companion-slot", "data-companion-slot": `block-${id}`, "data-companion-target": id, "data-box-id": id });
  // Readiness informs a prerequisite status. An empty block is not an error.
  if (window.companionBoxes) window.companionBoxes.readiness(id, { state: blockState, reason: done ? "Ready" : "Needs work" });
  return el("section", { class: `block${open ? " open" : ""}`, "data-state": blockState, "data-block": id, "data-box-id": id },
    action ? el("div", { class: "block-headrow" }, head, action) : head,
    slot,
    el("div", { class: "block-body" }, el("div", {}, el("div", { class: "block-inner" }, body))));
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
  updateAppFooter();
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
  // Song holds its lyrics and transcription; each keeps its own readiness light and state.
  const songState = observed("song")?.state || "unknown";
  const lyricsState = observed("lyrics")?.state || "unknown";
  const outputs = (transcript.outputs || []).filter((c) => c.exists !== false && c.path !== audio?.path);
  const transcriptState = observed("transcript")?.state || "unknown";
  const doc = transcript.document;
  box.append(block("song", ICONS.song, "Song", audio?.name, ready("song"),
    [el("div", { class: "why" }, song.why || ""), audio ? fileButton(audio) : null,
     songSection("lyrics", "Lyrics & notes", state.lyrics?.source === "canonical" ? `${state.lyrics.line_count} lines · ${state.lyrics.sections.length} sections` : "from transcript",
       ready("lyrics"), lyricsState, lyricsBody(state.lyrics, doc, liner, audio?.path)),
     songSection("transcript", "Transcription", transcriptSummary(doc, ready("transcript")), ready("transcript"), transcriptState,
       [doc?.words?.length ? el("button", { id: "song-open-transcript", class: "ghost", onclick: () => lyricReaderOpen(state.lyrics, doc, audio?.path, true) }, "Open transcript") : null,
        el("details", { class: "song-details" }, el("summary", {}, "Details"), el("div", { class: "why" }, transcript.why || ""), ...outputs.map(fileButton))])],
    songState, audio ? songPlayButton(audio.path) : null));

  const assets=state.data?.assets?.assets||[];
  const cards=assets.map(a=>{
    const picture=/\.(png|jpe?g|webp|gif)$/i.test(a.path||'')?el('img',{src:convertFileSrc(a.path),alt:a.name,draggable:'false'}):el('span',{html:ICONS.animate});
    const card=el('button',{id:`asset-${a.id}`,class:'object',draggable:'true',title:`${a.name} · drag onto a storyboard page or click to animate`,'aria-label':a.name,
      onclick:()=>openAnimation(a.id)},el('div',{class:'stack'},picture),el('span',{class:'label'},a.name));
    card.addEventListener('dragstart',e=>{e.dataTransfer.setData('application/x-magicstudio-asset',a.id);e.dataTransfer.setData('text/plain',a.id);e.dataTransfer.effectAllowed='copy';});return card;
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
  if (state.view === 'design' && typeof wordsStatusRefresh === 'function') wordsStatusRefresh();
}

// ---- Storyboard: title cards ------------------------------------------------------------

function renderStoryboard() {
  updateAppFooter();
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
      cards.append(el('div',{id:'sb-empty',class:'panel-empty'},el('strong',{},sb?.scene_packets_available?'Scene map available · editable deck not built':'Transcript available · storyboard not created'),el('p',{},'Your imported lyrics and timing are retained. Review them in Lyrics & notes. Storyboard generation is a separate step.'),el('button',{id:'sb-review-lyrics',class:'start','aria-label':'Review imported lyrics and timing',onclick:()=>{state.blocksCollapsed=false;state.open='lyrics';$('blocks').classList.remove('collapsed');renderBlocks();}},'Review lyrics & timing')));
      return;
    }
    cards.append(el("div", {id:"sb-empty", class:"panel-empty"},
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
  state.pageTitles = Object.fromEntries(nav.map(([label, page]) => [page, label]));
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
      const page=el("div",{id:`sb-sheet-${i}`,class:"sb-sheet"});
      page.style.setProperty('--ar',`${size[0]}/${size[1]}`);
      page.append(el("img",{src:convertFileSrc(path),alt:`Storyboard page ${i+1}`,loading:"lazy"}));
      for(const [j,link] of (sb.layout?.links||[]).entries())if(link.page===i+1){
        const [x,y,r,b]=link.rect;
        const linkButton=el("button",{id:`sb-link-${i}-${j}`,class:"sb-link",title:`Go to page ${link.to_page}`,"aria-label":`Go to page ${link.to_page}`,onclick:()=>showPage(link.to_page-1)});
        linkButton.style.setProperty('--x',`${x/size[0]*100}%`);
        linkButton.style.setProperty('--y',`${y/size[1]*100}%`);
        linkButton.style.setProperty('--w',`${(r-x)/size[0]*100}%`);
        linkButton.style.setProperty('--h',`${(b-y)/size[1]*100}%`);
        page.append(linkButton);
      }
      page.addEventListener('dragover',e=>{e.preventDefault();page.classList.add('drop-target');e.dataTransfer.dropEffect='copy';});
      page.addEventListener('dragleave',()=>page.classList.remove('drop-target'));
      page.addEventListener('drop',async e=>{
        e.preventDefault();page.classList.remove('drop-target');
        const id=e.dataTransfer.getData('application/x-magicstudio-asset');if(!id)return;
        const r=page.getBoundingClientRect();
        try{await designCommand('storyboard.place_asset',{asset:id,page:i+1,x:(e.clientX-r.left)/r.width*size[0]-80,y:(e.clientY-r.top)/r.height*size[1]-80,width:160});}catch(error){toast(String(error));}
      });
      wrap.append(page);
    });
    wrap.onscroll=()=>{let current=0;const top=wrap.getBoundingClientRect().top;[...wrap.children].forEach((p,i)=>{if(p.getBoundingClientRect().top<=top+wrap.clientHeight*.35)current=i;});state.page=current;$("sb-page").textContent=`${current+1} / ${pages.length}`;updateAppFooter();};
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
  if (view === "split") view = "studio";
  if (state.view === "animation" && view !== "animation" && typeof theatreCanLeave === "function" && !theatreCanLeave()) return false;
  if (typeof animationPause === "function") animationPause();
  if (view !== 'animation' && typeof blenderDispose === 'function') blenderDispose();
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
  else if (view === "design") { await invoke("studio_hide"); await wordsOpen(); }
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

// ---- Studio server: red = off (no memory used), green = running --------------------------

function serverUrl() {
  const server = state.data?.status?.video_server || {};
  return state.studioUrl || (server.running ? server.url : null);
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
      await motionCommand("video.init", {});
      state.data = await invoke("project_state", { path: state.project });
    }
    state.data = await invoke("studio_server", { path: state.project, on });
    if (!on) state.studioUrl = null;
    toast(on ? "Studio started" : "Studio stopped");
  } catch (e) { toast(String(e)); }
  renderTarget();
  if (["studio", "split"].includes(state.view)) (serverUrl() ? showStudio() : setView(state.view));
}

// ---- Startup ------------------------------------------------------------------------------

async function load() {
  await loadComponents();
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
  if (state.project !== path) { animationReset(); songStop(); transcriptReaderClose(); }
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
$("footer-server").addEventListener("click", toggleServer);
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
   'data-commands':'storyboard.edit',spellcheck:'true'});
  // Page geometry and the field's own colours are data; .sb-text turns them into layout.
  const put=(k,v)=>node.style.setProperty(k,v);
  put('--x',`${x/size[0]*100}%`);put('--y',`${y/size[1]*100}%`);put('--w',`${w/size[0]*100}%`);put('--h',`${(field.name?h:Math.max(h,50))/size[1]*100}%`);
  put('--font',`${(field.name?(field.font_size||12):(field.field==='lyrics'?27:21))/size[0]*100}cqw`);
  if(field.name){node.dataset.named='';put('--pad',`${2/size[0]*100}cqw`);put('--field-fill',field.fill);put('--field-ink',field.text_color);}
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
  const node=el('div',{id:item.id,class:'design-placement',title:'Drag to move · grab an edge to scale'});
  const place=(px,py,pw,ph)=>{node.style.setProperty('--x',`${100*px/size[0]}%`);node.style.setProperty('--y',`${100*py/size[1]}%`);if(pw!=null){node.style.setProperty('--w',`${100*pw/size[0]}%`);node.style.setProperty('--h',`${100*ph/size[1]}%`);}};
  place(x,y,w,h);
  node.append(el('img',{src:convertFileSrc(item.path),alt:'Placed design asset',draggable:'false'}));
  for(const [hx,hy] of [[-1,-1],[0,-1],[1,-1],[-1,0],[1,0],[-1,1],[0,1],[1,1]]){
   const handle=el('button',{class:'design-handle',title:'Drag to scale', 'aria-label':'Scale placed image','data-hx':hx,'data-hy':hy});
   handle.addEventListener('pointerdown',e=>{
    e.preventDefault();e.stopPropagation();selectedPlacement=item.id;handle.setPointerCapture(e.pointerId);
    const r=page.getBoundingClientRect(),start=[e.clientX,e.clientY];let rect=[x,y,w,h];
    handle.onpointermove=event=>{
     const dx=(event.clientX-start[0])*size[0]/r.width,dy=(event.clientY-start[1])*size[1]/r.height;
     const factor=hx===0?1+hy*dy/h:hy===0?1+hx*dx/w:1+(hx*dx/w+hy*dy/h)/2;
     const nw=Math.max(20,Math.min(size[0],w*factor)),nh=nw*h/w;
     rect=[hx<0?x+w-nw:hx===0?x+(w-nw)/2:x,hy<0?y+h-nh:hy===0?y+(h-nh)/2:y,nw,nh];
     place(rect[0],rect[1],nw,nh);
    };
    handle.onpointerup=async()=>{handle.onpointermove=null;handle.onpointerup=null;try{await designCommand('storyboard.place_asset',{asset:item.asset,page:item.page,placement:item.id,x:rect[0],y:rect[1],width:rect[2]});}catch(error){toast(String(error));}};
   });node.append(handle);
  }
  node.addEventListener('pointerdown',e=>{
   selectedPlacement=item.id;
   e.preventDefault();node.setPointerCapture(e.pointerId);const r=page.getBoundingClientRect(),start=[e.clientX,e.clientY];let nx=x,ny=y;
   node.onpointermove=event=>{nx=x+(event.clientX-start[0])*size[0]/r.width;ny=y+(event.clientY-start[1])*size[1]/r.height;place(nx,ny);};
   node.onpointerup=async()=>{node.onpointermove=null;node.onpointerup=null;try{await designCommand('storyboard.place_asset',{asset:item.asset,page:item.page,x:nx,y:ny,width:w,placement:item.id});}catch(error){toast(String(error));}};
  });const remove=el('button',{class:'design-remove',title:'Remove this placement','aria-label':'Remove this placement',onclick:()=>designCommand('storyboard.remove_placement',{placement:item.id}).catch(e=>toast(String(e)))},'×');
  remove.style.setProperty('--x',`${100*(x+w)/size[0]}%`);remove.style.setProperty('--y',`${100*y/size[1]}%`);page.append(node,remove);
 }
}
$('sb-save-version').addEventListener('click',async()=>{const button=$('sb-save-version');button.disabled=true;button.setAttribute('aria-busy','true');try{await designCommand('storyboard.save_version',state.data?.storyboard?.draft?{draft:state.data.storyboard.manifest}:{});toast('Saved a new editable storyboard version');}catch(e){toast(String(e));}finally{button.disabled=false;button.removeAttribute('aria-busy');}});
