// The agent drawer: chat with the project's opencode session (or an agent in Herdr), answer its
// permission requests and questions, and point at anything in the app to attach a picture of it.
// Same session and payloads as the native app's agent pane (<project>/.agent/opencode-session).
const SESSION = "session";
const agent = { open: false, session: null, target: SESSION, models: [], herdr: [], messages: [], permissions: [], forms: [],
  attachments: [], polling: false, timer: null, loaded: false };

const oc = (args, data) => invoke("opencode", { project: state.project, args, data: data ?? null });
const status = (text) => { $("agent-status").textContent = text; $("agent-status").title = text; };

async function agentLoad() {
  agent.loaded = true;
  agent.session = await invoke("agent_session", { project: state.project, set: null }).catch(() => null);
  const [models, herdr] = await Promise.allSettled([oc(["model.list"]), invoke("herdr", { args: ["agent", "list"] })]);
  agent.models = models.value?.data || [];
  agent.herdr = (herdr.value?.result?.agents || []).map((a) => ({
    id: a.name || a.pane_id,
    label: a.name ? `${a.name} (${a.agent || "agent"}, Herdr)` : `${a.agent || "agent"} in ${a.pane_id} (Herdr)`,
  }));
  const target = $("agent-target");
  target.replaceChildren(el("option", { value: SESSION }, "opencode session (this project)"),
    ...agent.herdr.map((h) => el("option", { value: h.id }, h.label)));
  target.value = agent.target;
  renderModels();
  renderWatch();
}

function currentModel() {
  for (let i = agent.messages.length - 1; i >= 0; i--) {
    const m = agent.messages[i].model;
    if (m?.providerID && m?.id) return m;
  }
  return null;
}

function renderModels() {
  const select = $("agent-model");
  select.hidden = agent.target !== SESSION;
  const cur = currentModel();
  const key = (p, id) => `${p}/${id}`;
  const opts = agent.models.map((m) => el("option", { value: key(m.providerID, m.id) }, `${m.name || m.id}  ·  ${m.providerID}`));
  if (!cur) opts.unshift(el("option", { value: "" }, "default model"));
  select.replaceChildren(...opts);
  select.value = cur ? key(cur.providerID, cur.id) : "";
}

function renderWatch() {
  const w = $("agent-watch");
  w.textContent = agent.session ? `watch: opencode --session ${agent.session}` : "";
  w.hidden = !agent.session;
}

async function agentPoll() {
  if (!agent.open || !agent.session || agent.polling || agent.target !== SESSION) return;
  agent.polling = true;
  const sid = `sessionID=${agent.session}`;
  try {
    const [messages, permissions, forms] = await Promise.allSettled([
      oc(["session.message.list", "--param", sid, "--param", "order=desc", "--param", "limit=40"]),
      oc(["session.permission.list", "--param", sid]),
      oc(["session.form.list", "--param", sid]),
    ]);
    if (messages.status === "fulfilled") agent.messages = (messages.value?.data || []).slice().reverse();
    else status(String(messages.reason));
    agent.permissions = permissions.value?.data || [];
    agent.forms = forms.value?.data || [];
    renderLog();
  } finally { agent.polling = false; }
}

function renderLog() {
  const log = $("agent-log");
  const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 40;
  const items = [];
  if (agent.target !== SESSION) {
    items.push(el("div", { class: "msg done" }, "Messages go to that agent's pane in Herdr; watch the reply there."));
  } else if (!agent.messages.length) {
    items.push(el("div", { class: "msg done" }, "Ask the agent anything about this project. Point at something to show it what you mean."));
  }
  if (agent.target === SESSION) {
    for (const m of agent.messages) {
      if (m.type === "user") items.push(el("div", { class: "msg user" }, m.text || ""));
      else if (m.type === "idle") items.push(el("div", { class: "msg done" }, "— done"));
      else if (m.type === "assistant") {
        for (const part of m.content || []) {
          if (part.type === "text" && part.text) items.push(el("div", { class: "msg assistant" }, part.text));
          else if (part.type?.includes("tool")) items.push(el("div", { class: "msg tool" }, `· ${part.name || part.tool || part.type}`));
        }
      }
    }
    for (const p of agent.permissions) items.push(permissionCard(p));
    for (const f of agent.forms) for (const field of f.fields || []) items.push(questionCard(f, field));
  }
  log.replaceChildren(...items);
  if (atBottom) log.scrollTop = log.scrollHeight;
  renderModels();
}

function permissionCard(p) {
  const what = `${p.action || ""} ${(p.resources || []).join(", ")}`.trim();
  const reply = (decision) => async () => {
    try {
      await oc(["session.permission.reply", "--param", `sessionID=${p.sessionID}`, "--param", `requestID=${p.id}`], { decision });
      status(`Permission: ${decision}.`);
    } catch (e) { status(String(e)); }
    agentPoll();
  };
  return el("div", { class: "card" }, el("div", { class: "q" }, "Permission"), el("div", {}, what),
    el("div", { class: "opts" },
      el("button", { class: "opt", onclick: reply("once") }, "Allow once"),
      el("button", { class: "opt", onclick: reply("always") }, "Always"),
      el("button", { class: "opt", onclick: reply("reject") }, "Reject")));
}

function questionCard(form, field) {
  const answer = async (value) => {
    if (!value) return;
    try {
      await oc(["session.form.reply", "--param", `sessionID=${form.sessionID}`, "--param", `formID=${form.id}`], { answer: { [field.key]: value } });
      status("Answered.");
    } catch (e) { status(String(e)); }
    agentPoll();
  };
  const card = el("div", { class: "card" }, el("div", { class: "q" }, field.title || "Question"));
  if (field.description) card.append(el("div", {}, field.description));
  const opts = (field.options || []).map((o) =>
    el("button", { class: "opt", title: o.description || "", onclick: () => answer(o.value) }, o.label || o.value || ""));
  if (opts.length) card.append(el("div", { class: "opts" }, opts));
  if (field.custom !== false) {
    const input = el("input", { placeholder: "Your own answer · Enter" });
    input.addEventListener("keydown", (e) => { if (e.key === "Enter") answer(input.value.trim()); });
    card.append(input);
  }
  return card;
}

function renderAttachments() {
  $("agent-attachments").replaceChildren(...agent.attachments.map((a, i) =>
    el("div", { class: "att", title: a.path },
      el("img", { src: convertFileSrc(a.path), alt: "" }), el("span", {}, a.what),
      el("button", { "aria-label": "Remove", title: "Remove", onclick: () => { agent.attachments.splice(i, 1); renderAttachments(); } }, "×"))));
}

async function agentSend() {
  const input = $("agent-input");
  const text = input.value.trim();
  if (!text && !agent.attachments.length) return;
  const attachments = agent.attachments.splice(0);
  input.value = "";
  renderAttachments();
  if (agent.target !== SESSION) {
    const body = [text, ...attachments.map((a) => `[Pointed at ${a.what} in AnimationCraft — picture: ${a.path}]`)].join("\n");
    status(`Sending to ${agent.target} in Herdr…`);
    try { await invoke("herdr", { args: ["agent", "prompt", agent.target, body] }); status(`Sent to ${agent.target} in Herdr.`); }
    catch (e) { status(`Herdr: ${e}`); }
    return;
  }
  status("Sending…");
  try {
    if (!agent.session) {
      const name = state.project.split("/").pop().replace(/\.acproj$/, "");
      const made = await oc(["session.create"], { title: `AnimationCraft · ${name}` });
      const id = made?.data?.id || made?.id;
      if (!id) throw new Error("opencode did not return a session id");
      agent.session = await invoke("agent_session", { project: state.project, set: id });
      renderWatch();
    }
    const files = attachments.map((a) => ({ uri: `file://${a.path}`, name: a.path.split("/").pop(), description: a.what }));
    const full = [text, ...attachments.map((a) => `[Pointed at ${a.what} in AnimationCraft; picture attached]`)].join("\n");
    await oc(["session.prompt", "--param", `sessionID=${agent.session}`], { text: full, files });
    status("Sent. The agent is working…");
  } catch (e) { status(String(e)); }
  agentPoll();
}

// Pointing: the overlay rings the element under the mouse; a click snapshots that rectangle.
function describe(node) {
  const label = node.getAttribute("aria-label") || node.title || node.closest("[aria-label]")?.getAttribute("aria-label");
  const text = (node.innerText || "").trim().split("\n")[0].slice(0, 60);
  const where = node.closest("section, .block, header, aside")?.getAttribute("aria-label") || node.closest(".block")?.querySelector("h2, .title")?.textContent;
  return [label || text || node.tagName.toLowerCase(), where && where !== label ? `in ${where.trim()}` : ""].filter(Boolean).join(" ");
}

function pick(x, y) {
  const overlay = $("point-overlay");
  overlay.style.pointerEvents = "none";
  const node = document.elementFromPoint(x, y);
  overlay.style.pointerEvents = "";
  return node && !node.closest("#agent") ? node : null;
}

function pointStart() {
  const overlay = $("point-overlay"), box = $("point-box");
  overlay.classList.add("on");
  $("agent").classList.remove("open");
  let target = null;
  const move = (e) => {
    target = pick(e.clientX, e.clientY);
    if (!target) { box.style.width = "0"; return; }
    const r = target.getBoundingClientRect();
    Object.assign(box.style, { left: `${r.left - 3}px`, top: `${r.top - 3}px`, width: `${r.width + 6}px`, height: `${r.height + 6}px` });
  };
  const stop = () => {
    overlay.classList.remove("on");
    overlay.removeEventListener("mousemove", move);
    overlay.removeEventListener("click", click);
    document.removeEventListener("keydown", esc, true);
    box.style.width = "0";
    $("agent").classList.add("open");
  };
  const click = async (e) => {
    e.preventDefault();
    const node = target || pick(e.clientX, e.clientY);
    stop();
    if (!node) return;
    const r = node.getBoundingClientRect(), pad = 12;
    const rect = [Math.max(0, r.left - pad), Math.max(0, r.top - pad),
      Math.min(innerWidth, r.right + pad) - Math.max(0, r.left - pad), Math.min(innerHeight, r.bottom + pad) - Math.max(0, r.top - pad)];
    status("Capturing…");
    // Let the overlay and drawer leave the picture before WebKit takes it.
    await new Promise((ok) => setTimeout(ok, 380));
    try {
      const path = await invoke("snapshot", { project: state.project, rect });
      agent.attachments.push({ path, what: describe(node) });
      renderAttachments();
      status("Picture attached.");
      $("agent-input").focus();
    } catch (err) { status(String(err)); }
  };
  const esc = (e) => { if (e.key === "Escape") { e.stopPropagation(); stop(); } };
  overlay.addEventListener("mousemove", move);
  overlay.addEventListener("click", click);
  document.addEventListener("keydown", esc, true);
}

function agentToggle(open = !agent.open) {
  if (!state.project) return toast("Open a project first");
  agent.open = open;
  $("agent").classList.toggle("open", open);
  $("agent").setAttribute("aria-hidden", String(!open));
  $("agent-toggle").classList.toggle("on", open);
  clearInterval(agent.timer);
  if (!open) return;
  (agent.loaded ? Promise.resolve() : agentLoad()).then(agentPoll);
  agent.timer = setInterval(agentPoll, 2000);
  $("agent-input").focus();
}

// Pictures pasted (Ctrl+V) or dropped onto the drawer attach like pointed ones.
async function attachFiles(files) {
  for (const file of files) {
    if (!file.type.startsWith("image/")) { status(`Only pictures attach (${file.name || file.type} skipped).`); continue; }
    try {
      const name = file.name && file.name !== "image.png" ? file.name : `image.${file.type.split("/")[1] || "png"}`;
      const path = await invoke("save_attachment", new Uint8Array(await file.arrayBuffer()),
        { headers: { "x-project": state.project, "x-name": encodeURIComponent(name) } });
      agent.attachments.push({ path, what: file.name && file.name !== "image.png" ? file.name : "a pasted picture" });
      renderAttachments();
      status("Picture attached.");
    } catch (err) { status(String(err)); }
  }
}

$("agent-input").addEventListener("paste", (e) => {
  const files = [...(e.clipboardData?.files || [])];
  if (files.length) { e.preventDefault(); attachFiles(files); }
});
$("agent").addEventListener("dragover", (e) => { e.preventDefault(); $("agent").classList.add("drop"); });
$("agent").addEventListener("dragleave", (e) => { if (!$("agent").contains(e.relatedTarget)) $("agent").classList.remove("drop"); });
$("agent").addEventListener("drop", (e) => {
  e.preventDefault();
  $("agent").classList.remove("drop");
  attachFiles([...(e.dataTransfer?.files || [])]);
});
$("agent-toggle").addEventListener("click", () => agentToggle());
$("agent-close").addEventListener("click", () => agentToggle(false));
$("agent-send").addEventListener("click", agentSend);
$("agent-point").addEventListener("click", pointStart);
$("agent-input").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); agentSend(); }
});
$("agent-target").addEventListener("change", (e) => { agent.target = e.target.value; renderLog(); agentPoll(); });
$("agent-model").addEventListener("change", async (e) => {
  const [providerID, ...rest] = e.target.value.split("/");
  if (!providerID || !agent.session) return;
  try {
    await oc(["session.switchModel", "--param", `sessionID=${agent.session}`], { model: { id: rest.join("/"), providerID } });
    status("Model switched.");
  } catch (err) { status(String(err)); }
});
$("agent-watch").addEventListener("click", async () => {
  try { await navigator.clipboard.writeText(`opencode --session ${agent.session}`); toast("Copied"); } catch { toast("Copy failed"); }
});
document.addEventListener("keydown", (e) => {
  if (e.key === "/" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); agentToggle(); return; }
  if (e.key === "Escape" && agent.open && !$("point-overlay").classList.contains("on")) agentToggle(false);
});
