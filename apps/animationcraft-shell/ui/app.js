// AnimationCraft shell: the project view. Data comes from animationcraft-engine through the
// shell's commands; files open with one click; the studio tab hosts the video target's studio.
const { invoke, convertFileSrc } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);

const ICONS = {
  song: '<svg viewBox="0 0 24 24"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>',
  lyrics: '<svg viewBox="0 0 24 24"><path d="M4 6h16M4 11h16M4 16h10"/></svg>',
  transcript: '<svg viewBox="0 0 24 24"><path d="M3 12h2M7 8v8M11 5v14M15 9v6M19 11v2"/></svg>',
  animate: '<svg viewBox="0 0 24 24"><path d="M12 3l9 5-9 5-9-5 9-5z"/><path d="M3 13l9 5 9-5"/></svg>',
  chev: '<svg viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
};

let state = { project: null, studioUrl: null, data: null, page: 0, open: "song", view: "project" };

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
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), v);
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

function block(id, icon, title, meta, done, body) {
  const open = state.open === id;
  const head = el("button", { class: "block-head", "aria-expanded": String(open), title: `${title}${meta ? " · " + meta : ""}`,
      onclick: () => { state.open = open ? null : id; renderBlocks(); } },
    el("span", { class: `dot ${done ? "ok" : "warn"}`, title: done ? "Ready" : "Needs work" }),
    el("span", { class: "ico", html: icon }),
    el("span", { class: "title" }, title),
    el("span", { class: "meta" }, meta || ""),
    el("span", { class: "chev", html: ICONS.chev }));
  return el("section", { class: `block${open ? " open" : ""}` }, head,
    el("div", { class: "block-body" }, el("div", {}, el("div", { class: "block-inner" }, body))));
}

function sceneLines() {
  // "Scene 3 · arm. · 0:06–0:07" -> {time, lyric}
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
      onclick: () => toast(`${name}: ${layers.length} layers — opening in the studio comes next`) },
    stack, el("span", { class: "label" }, name), el("span", { class: "count" }, `${layers.length} layer${layers.length === 1 ? "" : "s"}`));
}

function renderBlocks() {
  const s = state.data?.status;
  const box = $("blocks");
  box.replaceChildren();
  if (!s) return;
  const stage = (id) => s.stages.find((x) => x.id === id) || { inputs: [], outputs: [] };
  const song = stage("song"), transcript = stage("transcript"), animate = stage("animate");
  const audio = song.inputs?.[0];
  const liner = (song.inputs || []).slice(1);

  box.append(block("song", ICONS.song, "Song", audio?.name, song.done,
    [el("div", { class: "why" }, song.why || ""), audio ? fileButton(audio) : null]));

  const lines = sceneLines();
  box.append(block("lyrics", ICONS.lyrics, "Lyrics & notes", `${lines.filter((l) => l.lyric && l.lyric !== "Instrumental").length} lines`, lines.length > 0,
    [...lines.map((l) => el("div", { class: "lyric" }, el("span", { class: "t" }, l.time), el("span", {}, l.lyric))),
     el("div", { class: "group-label" }, "Liner & teaching notes"),
     liner.length ? liner.map(fileButton) : el("div", { class: "why" }, "None in this project yet.")]));

  const outputs = (transcript.outputs || []).filter((c) => c.exists !== false && c.path !== audio?.path);
  box.append(block("transcript", ICONS.transcript, "Transcription", `${outputs.length} files`, transcript.done, outputs.map(fileButton)));

  const groups = objectsFrom(s.asset_files || []);
  const objects = [...groups].filter(([k]) => k !== "backgrounds" && k !== "loose");
  const body = [el("div", { class: "group-label" }, "Objects"),
    el("div", { class: "objects" }, objects.map(([name, layers]) => objectCard(name, layers, true)))];
  if (groups.has("backgrounds")) body.push(el("div", { class: "group-label" }, "Backgrounds"),
    el("div", { class: "objects" }, groups.get("backgrounds").map((f) => objectCard(f.name.replace(/\.[^.]+$/, ""), [f], false))));
  if (groups.has("loose")) body.push(el("div", { class: "group-label" }, "Not grouped yet"), groups.get("loose").map(fileButton));
  box.append(block("animate", ICONS.animate, "Animate", `${objects.length} objects`, animate.done, body));
}

// ---- Storyboard ---------------------------------------------------------------------------

function renderStoryboard() {
  const sb = state.data?.storyboard;
  const pages = sb?.previews || [];
  const chips = $("sb-chips");
  chips.replaceChildren();
  if (!pages.length) { $("sb-page").textContent = "no storyboard yet"; return; }
  const first = (sb.first_scene_page || 2) - 1;
  const nav = [["Lyrics", 0], ["Cast & sets", 1],
    ...(sb.scene_titles || []).map((t, i) => [t.split(" · ").slice(0, 2).join(" · "), first + i]), ["Brief", pages.length - 1]];
  for (const [label, page] of nav) {
    const active = page === state.page;
    chips.append(el("button", { class: `chip${active ? " active" : ""}`, role: "tab", "aria-selected": String(active), title: label,
      onclick: () => showPage(page) }, label));
  }
  chips.querySelector(".active")?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  $("sb-page").textContent = `${state.page + 1} / ${pages.length}`;
  const img = $("sb-img");
  img.classList.add("loading");
  img.onload = () => img.classList.remove("loading");
  img.onerror = () => toast(`Could not load ${img.src}`);
  img.src = convertFileSrc(pages[state.page]);
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
  catch (e) { toast(String(e)); }
}

async function setView(view) {
  state.view = view;
  document.querySelectorAll(".tab").forEach((t) => {
    const on = t.dataset.view === view;
    t.classList.toggle("active", on);
    t.setAttribute("aria-selected", String(on));
  });
  // Split keeps the project view on top; the studio fills the space under it.
  const page = view === "split" ? "project" : view;
  document.body.classList.toggle("split", view === "split");
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${page}`));
  placeInk();
  if (view !== "project" && serverUrl()) requestAnimationFrame(showStudio);
  else {
    await invoke("studio_hide");
    // With the studio off, Split and Studio show the calm "start it" card instead.
    if (view !== "project") {
      document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-studio"));
      document.body.classList.remove("split");
      $("studio-empty").classList.add("show");
    }
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
  if (state.view !== "project") (serverUrl() ? showStudio() : setView(state.view));
}

// ---- Startup ------------------------------------------------------------------------------

async function load() {
  const start = await invoke("startup");
  state.project = start.project || localStorage.getItem("project");
  state.studioUrl = start.studio || null;
  if (start.open) state.open = start.open;
  if (!state.project) { $("project-name").textContent = "Open a project: animationcraft-shell <project.acproj>"; return; }
  localStorage.setItem("project", state.project);
  $("project-name").textContent = state.project.split("/").pop().replace(/\.acproj$/, "");
  try { state.data = await invoke("project_state", { path: state.project }); }
  catch (e) { toast(String(e)); return; }
  renderTarget();
  renderBlocks();
  renderStoryboard();
  if (start.view) setView(start.view);
}

document.querySelectorAll(".tab").forEach((t) => t.addEventListener("click", () => setView(t.dataset.view)));

// The frame between the project view and the studio: drag to move, double-click for half,
// buttons collapse either side (studio only / project only).
(() => {
  const handle = $("frame-handle");
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
load();
