// The agent drawer: chat with the project's opencode session (or an agent in Herdr), answer its
// permission requests and questions, and point at anything in the app to attach a picture of it.
// Same session and payloads as the native app's agent pane (<project>/.agent/opencode-session).
const SESSION = "session";
const agent = { open: false, session: null, target: SESSION, models: [], herdr: [], messages: [], permissions: [], forms: [],
  attachments: [], polling: false, timer: null, loaded: false, sending: false };

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
  const wanted = agent.models.map((m) => ({ value: key(m.providerID, m.id), label: `${m.name || m.id}  ·  ${m.providerID}` }));
  if (!cur) wanted.unshift({ value: "", label: "default model" });
  // Rebuild the options only when the list really changed: replacing them every poll closed an
  // open menu and churned the DOM every two seconds.
  const have = [...select.options].map((o) => o.value);
  if (have.length !== wanted.length || have.some((v, i) => v !== wanted[i].value)) {
    select.replaceChildren(...wanted.map((o) => el("option", { value: o.value }, o.label)));
  }
  const value = cur ? key(cur.providerID, cur.id) : "";
  if (select.value !== value && document.activeElement !== select) select.value = value;
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
    if (sid !== `sessionID=${agent.session}`) return;
    if (messages.status === "fulfilled") agent.messages = (messages.value?.data || []).slice().reverse();
    else status(String(messages.reason));
    agent.permissions = permissions.value?.data || [];
    agent.forms = forms.value?.data || [];
    renderLog();
  } finally { agent.polling = false; }
}

// The log is polled every two seconds. It is described here as keyed items, not DOM, so the
// render below can keep the nodes it already has: rebuilding the log replayed the entry
// animation on the whole conversation, dropped the reader's scroll and selection, and threw away
// an answer being typed into a permission or question card.
function logItems() {
  const items = [];
  if (agent.target !== SESSION) {
    items.push({ key: "notice:herdr", cls: "msg done", text: "Messages go to that agent's pane in Herdr; watch the reply there." });
  } else if (!agent.messages.length) {
    items.push({ key: "notice:empty", cls: "msg done", text: "Ask the agent anything about this project. Point at something to show it what you mean." });
  }
  if (agent.target === SESSION) {
    for (const m of agent.messages) {
      const id = m.id ?? m.role;
      if (m.type === "user") items.push({ key: `u:${id}`, cls: "msg user", text: m.text || "" });
      else if (m.type === "idle") items.push({ key: `i:${id}`, cls: "msg done", text: "— done" });
      else if (m.type === "assistant") {
        (m.content || []).forEach((part, i) => {
          if (part.type === "text" && part.text) items.push({ key: `a:${id}:${i}`, cls: "msg assistant", text: part.text });
          else if (part.type?.includes("tool")) items.push({ key: `t:${id}:${i}`, cls: "msg tool", text: `· ${part.name || part.tool || part.type}` });
        });
      }
    }
    for (const p of agent.permissions) items.push({ key: `p:${p.sessionID}:${p.id}`, build: () => permissionCard(p) });
    for (const f of agent.forms) for (const field of f.fields || []) items.push({ key: `q:${f.sessionID}:${f.id}:${field.key}`, build: () => questionCard(f, field) });
  }
  return items;
}

function renderLog() {
  const log = $("agent-log");
  const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 40;
  const top = log.scrollTop;

  const existing = new Map();
  for (const child of log.children) if (child.dataset?.k) existing.set(child.dataset.k, child);

  const next = logItems().map((item) => {
    const node = existing.get(item.key);
    if (!node) {
      const made = item.build ? item.build() : el("div", { class: item.cls }, item.text);
      made.dataset.k = item.key;
      return made;
    }
    existing.delete(item.key);
    // Same message, more text: update it in place, so the node keeps its place in the log.
    if (item.text !== undefined && node.textContent !== item.text) node.textContent = item.text;
    return node;
  });

  for (const gone of existing.values()) gone.remove();
  for (let i = 0; i < next.length; i++) {
    const node = next[i];
    if (node.parentNode === log) continue;
    // Only new nodes are inserted: moving a node that is already there restarts its animation.
    let anchor = null;
    for (let j = i + 1; j < next.length; j++) if (next[j].parentNode === log) { anchor = next[j]; break; }
    log.insertBefore(node, anchor);
  }

  if (atBottom) log.scrollTop = log.scrollHeight;
  else log.scrollTop = top;
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

// The draft and its pictures are handed over only once the message is delivered: a failed send
// keeps them, and anything typed or attached while it was in flight stays in the composer.
function clearSent(input, raw, attachments) {
  if (input.value === raw) input.value = "";
  const sent = new Set(attachments);
  if (agent.attachments.some((a) => sent.has(a))) {
    agent.attachments = agent.attachments.filter((a) => !sent.has(a));
    renderAttachments();
  }
}

async function agentNewChat(){
 if(agent.sending)throw new Error("Wait for the current message to finish sending");
 const button=$("agent-new");button.disabled=true;
 try{
  const result=await motionCommand("agent.new_chat",{model:currentModel()});
  agent.session=result.session;agent.target=SESSION;agent.messages=[];agent.permissions=[];agent.forms=[];
  $("agent-target").value=SESSION;renderLog();renderWatch();renderModels();
  status("New chat · previous conversation saved");$("agent-input").focus();
  return result;
 }finally{button.disabled=false;}
}
$("agent-new").addEventListener("click",()=>agentNewChat().catch(e=>status(String(e))));

async function agentSend() {
  const input = $("agent-input");
  if (agent.sending) return;
  const raw = input.value;
  const text = raw.trim();
  if (!text && !agent.attachments.length) return;
  const attachments = agent.attachments.slice();
  agent.sending = true;
  $("agent-send").disabled = true;
  try {
    if (agent.target !== SESSION) {
      const body = [text, ...attachments.map((a) => `[Pointed at ${a.what} in AnimationCraft — picture: ${a.path}]`)].join("\n");
      status(`Sending to ${agent.target} in Herdr…`);
      await invoke("herdr", { args: ["agent", "prompt", agent.target, body] });
      status(`Sent to ${agent.target} in Herdr.`);
    } else {
      status("Sending…");
      if (!agent.session) { const made=await motionCommand("agent.new_chat",{});agent.session=made.session;renderWatch(); }
      const files = attachments.map((a) => ({ uri: `file://${a.path}`, name: a.path.split("/").pop(), description: a.what }));
      const full = [text, ...attachments.map((a) => `[Pointed at ${a.what} in AnimationCraft; picture attached]`)].join("\n");
      await oc(["session.prompt", "--param", `sessionID=${agent.session}`], { text: full, files });
      status("Sent. The agent is working…");
    }
    clearSent(input, raw, attachments);
    agentPoll();
  } catch (e) {
    status(String(e));
    input.focus();
  } finally {
    agent.sending = false;
    $("agent-send").disabled = false;
  }
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
  document.body.classList.toggle("agent-docked",open);
  invoke("agent_dock",{width:open?$("agent").getBoundingClientRect().width:0}).catch(e=>status(String(e)));
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
  // An IME candidate is not a send: Enter only picks the candidate.
  if (e.isComposing || e.key === "Process") return;
  if (e.key !== "Enter") return;
  if (e.shiftKey) return; // Shift+Enter is a line break.
  e.preventDefault();
  agentSend();
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

window.addEventListener("resize",()=>{ if(agent.open) invoke("agent_dock",{width:$("agent").getBoundingClientRect().width}).catch(()=>{}); });
