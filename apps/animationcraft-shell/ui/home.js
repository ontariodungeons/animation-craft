// Home: shown when no project is open (and from the brand mark). New song, Open, and the recent
// projects as cards. Opening and creating run the engine's `project.open` / `project.new`.
const dialog = window.__TAURI__.dialog;

const FILM = '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4"/></svg>';
const CROSS = '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>';

async function showHome() {
  if (agent.open) agentToggle(false);
  Object.assign(agent, { loaded: false, session: null, messages: [], permissions: [], forms: [], attachments: [] });
  state.project = null;
  state.data = null;
  document.body.classList.add("at-home");
  document.body.classList.remove("split");
  $("project-name").textContent = "AnimationCraft";
  document.title = "AnimationCraft";
  document.querySelectorAll(".view").forEach((v) => v.classList.toggle("active", v.id === "view-home"));
  await invoke("studio_hide").catch(() => {});
  renderRecent();
}

async function renderRecent() {
  const list = await invoke("recent_projects");
  $("recent-label").hidden = !list.length;
  if (!list.length) {
    $("recent").replaceChildren(el("div", { class: "home-empty" }, "No projects yet. Start with New song."));
    return;
  }
  $("recent").replaceChildren(...list.map((p) => {
    const thumb = el("div", { class: "thumb", html: p.thumb ? "" : FILM });
    if (p.thumb) thumb.style.backgroundImage = `url("${convertFileSrc(p.thumb)}")`;
    const forget = el("span", { class: "forget", role: "button", tabindex: "0", "aria-label": `Remove ${p.name} from Recent`,
      title: "Remove from Recent (the files stay)", html: CROSS });
    forget.addEventListener("click", async (e) => {
      e.stopPropagation();
      await invoke("recent_forget", { path: p.path });
      renderRecent();
    });
    return el("button", { class: `proj${p.exists ? "" : " missing"}`, title: p.exists ? p.path : `Not found: ${p.path}`,
      onclick: () => p.exists ? openProject(p.path) : toast("That project was moved or deleted") },
      thumb, forget,
      el("div", { class: "meta" }, el("span", { class: "name" }, p.name),
        el("span", { class: "folder" }, p.exists ? `‎${p.folder}` : "moved or deleted")));
  }));
}

async function openDialog() {
  const path = await dialog.open({ title: "Open project", filters: [{ name: "AnimationCraft project", extensions: ["acproj"] }] });
  if (path) openProject(path);
}

// ---- New song sheet -------------------------------------------------------------------------

function slug(name) {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}

function renderWhere() {
  const name = $("new-name").value.trim(), root = $("new-root").value.trim();
  $("new-where").textContent = name && root ? `Creates ${root}/${slug(name)}/${name}.acproj` : "";
}

async function newSheet(open = true) {
  $("new-sheet").classList.toggle("open", open);
  $("new-sheet").setAttribute("aria-hidden", String(!open));
  if (!open) return;
  if (!$("new-root").value) $("new-root").value = (await invoke("projects_root")) || "";
  renderWhere();
  $("new-name").focus();
}

$("new-audio-pick").addEventListener("click", async () => {
  const file = await dialog.open({ title: "Song audio", filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "aac", "ogg"] }] });
  if (!file) return;
  $("new-audio").value = file;
  if (!$("new-name").value.trim()) {
    $("new-name").value = file.split("/").pop().replace(/\.[^.]+$/, "").replace(/[._-]+/g, " ").replace(/\bcdda\b/i, "").trim();
  }
  renderWhere();
});
$("new-root-pick").addEventListener("click", async () => {
  const dir = await dialog.open({ title: "Where the project folder goes", directory: true, defaultPath: $("new-root").value || undefined });
  if (dir) { $("new-root").value = dir; renderWhere(); }
});
$("new-name").addEventListener("input", renderWhere);
$("new-cancel").addEventListener("click", () => newSheet(false));
$("new-sheet").addEventListener("click", (e) => { if (e.target === $("new-sheet")) newSheet(false); });
$("new-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  if (!$("new-audio").value) return toast("Choose the song's audio first");
  $("new-create").disabled = true;
  try {
    const path = await invoke("project_create", { name: $("new-name").value, audio: $("new-audio").value, root: $("new-root").value });
    newSheet(false);
    $("new-form").reset();
    await openProject(path);
    toast("Project created");
  } catch (err) { toast(String(err)); }
  $("new-create").disabled = false;
});

$("home-new").addEventListener("click", () => newSheet(true));
$("home-open").addEventListener("click", openDialog);
$("brand").addEventListener("click", showHome);
$("brand").addEventListener("keydown", (e) => { if (e.key === "Enter") showHome(); });
document.addEventListener("keydown", (e) => {
  if (!(e.ctrlKey || e.metaKey) || e.target.closest("input, textarea")) {
    if (e.key === "Escape" && $("new-sheet").classList.contains("open")) newSheet(false);
    return;
  }
  if (e.key === "n") { e.preventDefault(); showHome().then(() => newSheet(true)); }
  if (e.key === "o") { e.preventDefault(); openDialog(); }
});
