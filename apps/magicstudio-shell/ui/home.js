// Home: shown when no project is open (and from the brand mark). New song, Open, and the recent
// projects as cards. Opening and creating run the engine's `project.open` / `project.new`.
const dialog = window.__TAURI__.dialog;

const FILM = '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"/></svg>';
const CROSS = '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>';

async function showHome() {
  if (typeof theatreCanLeave === "function" && !theatreCanLeave()) return;
  if(state.project){try{await invoke("engine_command",{id:"project.close",params:{}});}catch(e){toast(String(e));throw e;}}
  state.creating = false;
  newProjectForm.hidden = true;
  animationReset();
  window.workshopDrawers?.close(false);
  Object.assign(agent, { loaded: false, session: null, messages: [], permissions: [], forms: [], attachments: [] });
  state.project = null;
  window.magicCompanions?.sync();
  state.data = null;
  document.body.classList.add("at-home");
  document.body.classList.remove("split");
  $("project-name").textContent = "No project";
  document.title = "Magic Studio";
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-home"));
  await invoke("studio_hide").catch(() => {});
  await renderRecent();
  requestAnimationFrame(()=>acFeedback.report('ready'));
}

async function renderRecent() {
  const list = await invoke("recent_projects");
  $("recent-label").hidden = !list.length;
  if (!list.length) {
    $("recent").replaceChildren(el("div", { class: "panel-empty boxed" }, "No projects yet. Start with New song."));
    return;
  }
  $("recent").replaceChildren(...list.map((p) => {
    const thumb = el("div", { class: "thumb", html: p.thumb ? "" : FILM });
    if (p.thumb) thumb.style.setProperty("--thumbnail-image", `url("${convertFileSrc(p.thumb)}")`);
    const forget = el("span", { class: "forget", role: "button", tabindex: "0", "aria-label": `Remove ${p.name} from Recent`,
      title: "Remove from Recent (the files stay)", html: CROSS });
    forget.addEventListener("click", async (e) => {
      e.stopPropagation();
      await invoke("recent_forget", { path: p.path });
      renderRecent();
    });
    const status=p.open_state||'unknown';
    const main=el('button',{class:'project-main',title:p.path,onclick:()=>p.exists?openProject(p.path):toast('That project was moved or deleted')},
      el('span',{class:'name'},p.name),el('span',{class:'folder'},p.exists?p.folder:'Moved or deleted'),
      el('span',{class:'project-state '+(status==='open'?'is-open':'needs-check')},status==='open'?'● Open in Studio':status==='not_observed_open'?'○ Not observed open':`△ ${status}`));
    const links=el('div',{class:'project-quick'});
    for(const action of p.readiness?.quick_actions||[]){links.append(el('button',{class:'ghost',...(action.enabled?{}:{disabled:''}),title:action.enabled?action.label:(p.readiness?.next_step||'Requirements not met'),onclick:()=>openProject(p.path,{view:action.view,open:action.open_block})},action.label));}
    return el('div',{class:`proj${p.exists?'':' missing'}`,title:p.path},thumb,forget,
      el('div',{class:'project-summary'},main,el('p',{class:'why'},p.readiness?.next_step||'Inspect project state'),links));
  }));
}

async function openDialog() {
  const path = await dialog.open({ title: "Open project", filters: [{ name: "Magic Studio project", extensions: ["magic"] }] });
  if (path) openProject(path);
}

// New projects use the existing workflow blocks.
const newProjectForm = $("new-form");

async function beginNewProject() {
  if (state.project) { await invoke("new_window"); return; }
  await showHome();
  Object.assign(state, {creating:true, open:"song", view:"project", blocksCollapsed:false});
  document.body.classList.remove("at-home");
  document.querySelectorAll(".view").forEach(v => v.classList.toggle("active", v.id === "view-project"));
  $("blocks").classList.remove("collapsed");
  $("project-name").textContent = "New project";
  $("sb-chips").replaceChildren();
  $("sb-page").textContent = "—";
  $("sb-cards").replaceChildren(projectSetupCard(true));
  renderBlocks();
  try { const productionRoot = await invoke("projects_root"); $("new-root").value = productionRoot; $("new-root").defaultValue = productionRoot; }
  catch (error) { $("new-root").value = ""; toast(String(error)); }
  $("new-name").focus();
}

function renderNewProjectBlocks(box) {
  newProjectForm.hidden = false;
  box.append(block("song", ICONS.song, "Song", "New project", false, [newProjectForm], "missing"));
  for (const [id, title] of [["storyboard", "Storyboard"], ["assets", "Assets"], ["animate", "Animate"]]) {
    box.append(block(id, ICONS.animate, title, "", false,
      [el("span", {class:"why"}, "After recording")], "missing"));
  }
}

$("new-audio-pick").addEventListener("click", async () => {
  const file = await dialog.open({ title: "Song audio", filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"] }] });
  if (!file) return;
  $("new-audio").value = file;
  if (!$("new-name").value.trim()) {
    $("new-name").value = file.split("/").pop().replace(/\.[^.]+$/, "").replace(/[._-]+/g, " ").replace(/\bcdda\b/i, "").trim();
  }
});
$("new-root-pick").addEventListener("click", async () => {
  const dir = await dialog.open({ title: "Folder for this project", directory: true, defaultPath: $("new-root").value || undefined });
  if (dir) { $("new-root").value = dir; }
});
$("new-cancel").addEventListener("click", () => showHome().catch(error => toast(String(error))));
$("new-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  if (!$("new-audio").value) return toast("Choose the song's audio first");
  $("new-create").disabled = true;
  try {
    const action=acFeedback.actions.get(e.submitter);
    const opened = await invoke("project_create", {request:action?.request_id,control:action?.control, name: $("new-name").value, audio: $("new-audio").value, root: $("new-root").value });
    state.creating = false;
    newProjectForm.hidden = true;
    newProjectForm.reset();
    await applyOpenedProject(opened);
    toast(opened.problems?.length ? `Project created · ${opened.problems.join('; ')}` : "Project created");
  } catch (err) { toast(String(err)); }
  newProjectForm.querySelector("#new-create").disabled = false;
});

$("home-new").addEventListener("click", () => beginNewProject().catch(error => toast(String(error))));
$("home-open").addEventListener("click", openDialog);
$("brand").addEventListener("click", () => showHome().catch(error => toast(String(error))));
$("brand").addEventListener("keydown", (e) => { if (e.key === "Enter") showHome(); });
document.addEventListener("keydown", (e) => {
  if (!(e.ctrlKey || e.metaKey) || e.target.closest("input, textarea")) {
    return;
  }
  if (e.key === "n") { e.preventDefault(); beginNewProject().catch(error => toast(String(error))); }
  if (e.key === "o") { e.preventDefault(); openDialog(); }
});
