// The agent drawer: the app's own agent.
//
// Its settings are stored by the app (`agent_config`), its conversation lives in the project
// (`.agent/agent-log.json`) and every turn runs the configured command as a child process of this
// window, in the project folder, with the app's instructions appended to its system prompt.
// OpenCode uses its existing installed API/session route; no image settings are changed here.
const agent = {
  open: false, turns: [], config: null, sending: false, attachments: [], loaded: false,
  newChat: true, models: [], timer: null, renderedReply: null,
};

const status = (text) => { $("agent-status").textContent = text; $("agent-status").title = text; };

// ---- Settings ------------------------------------------------------------------------------

function renderConfig() {
  const c = agent.config;
  const runtime = $("agent-runtime");
  const chosen = c?.agent || "";
  const isDefault = c?.default && c.default === chosen;
  runtime.textContent = c
    ? `${chosen || "no agent"}${isDefault ? " · system default" : ""}${c.provider ? ` · ${c.provider}` : ""}${c.model ? ` · ${c.model}` : " · CLI default (unverified)"}`
    : "agent not configured";
  const ready = !!c && (c.agents || []).some((a) => a.installed && a.name === chosen);
  $("agent-connection").textContent = ready ? "· connected" : "· not configured";
  $("agent-connection").title = ready ? `${chosen} is installed and selected` : "Open Settings to choose an installed agent";
  runtime.title = c
    ? `Installed agents: ${(c.agents || []).filter((a) => a.installed).map((a) => a.name).join(", ")}\n` +
      `System default: ${c.default || "none"}\nInstructions: ${c.instructions}\nExtra arguments: ${(c.extra_args || []).join(" ") || "none"}`
    : "Open settings to choose an installed agent.";
  // The list comes from the system: every installed agent, with the Omarchy default marked.
  const select = $("agent-command");
  const installed = (c?.agents || []).filter((a) => a.installed);
  const options = installed.map((a) =>
    el("option", { value: a.name }, `${a.name}${a.default ? " (system default)" : ""}${a.headless ? "" : " · no headless mode"}`));
  const signature = options.map((o) => o.value).join(",");
  if (select.dataset.signature !== signature) {
    select.replaceChildren(...options);
    select.dataset.signature = signature;
  }
  select.value = chosen;
  $("agent-command").title = installed.filter((a) => !a.headless).length
    ? "Agents without a headless mode cannot answer from this window yet"
    : "The system agent this app runs";
  $("agent-provider").value = c?.provider || "";
  $("agent-model").value = c?.model || "";
  $("agent-thinking").value = c?.thinking || "";
  $("agent-extra").value = (c?.extra_args || []).join(" ");
  // These values came from saved settings, not a user draft. Later edits still differ from this baseline.
  for (const id of ["agent-provider", "agent-model", "agent-extra"]) $(id).defaultValue = $(id).value;
}

async function configSave() {
  const patch = {
    agent: $("agent-command").value,
    provider: $("agent-provider").value.trim(),
    model: $("agent-model").value.trim(),
    thinking: $("agent-thinking").value,
    extra_args: $("agent-extra").value.trim().split(/\s+/).filter(Boolean),
  };
  try {
    agent.config = await invoke("agent_config_save", { patch });
    renderConfig();
    $("anim-agent-settings").hidden = true;
    $("agent-config-toggle").setAttribute("aria-expanded", "false");
    agent.newChat = true;
    status("Chat selection saved · next message starts fresh");
    await refreshModels();
  } catch (error) { status(String(error)); }
}

// Catalog values come from the installed OpenCode API, never a hard-coded model list.
async function refreshModels() {
  try {
    const reply = await motionCommand("agent.models", { agent: $("agent-command").value });
    agent.models = Array.isArray(reply.models) ? reply.models : [];
    const providers = [...new Set(agent.models.filter((m) => m.enabled).map((m) => m.providerID))];
    $("agent-providers").replaceChildren(...providers.map((p) => el("option", { value: p })));
    renderModelOptions();
    const local = agent.models.filter((m) => m.enabled && m.local).length;
    $("agent-catalog-status").textContent = reply.available
      ? `${agent.models.filter((m) => m.enabled).length} available models${local ? ` · ${local} local` : ""} · selection is checked before sending`
      : (reply.reason || "Catalog unavailable; no model substituted");
  } catch (error) {
    agent.models = [];
    $("agent-providers").replaceChildren();
    $("agent-models").replaceChildren();
    $("agent-catalog-status").textContent = String(error);
  }
}
function renderModelOptions() {
  const provider = $("agent-provider").value.trim();
  // Local models first: they run on this machine, cost nothing and have no quota.
  const options = agent.models.filter((m) => m.enabled && m.providerID === provider)
    .slice().sort((a, b) => (b.local ? 1 : 0) - (a.local ? 1 : 0) || String(a.id).localeCompare(String(b.id)));
  $("agent-models").replaceChildren(...options.map((m) => el("option", { value: m.id, title: m.local ? "Local model — runs on this machine" : "" },
    `${m.name || m.id}${m.local ? " · local" : ""}`)));
}

// ---- Conversation --------------------------------------------------------------------------

async function agentLoad() {
  agent.loaded = true;
  const [config, turns] = await Promise.allSettled([
    invoke("agent_config"),
    state.project ? motionCommand("agent.log") : Promise.resolve([]),
  ]);
  if (config.status === "fulfilled") agent.config = config.value;
  else status(String(config.reason));
  agent.turns = turns.status === "fulfilled" && Array.isArray(turns.value) ? turns.value : [];
  renderConfig();
  renderLog();
}

async function agentReloadLog() {
  if (!state.project) return;
  try {
    const turns = await motionCommand("agent.log",{refresh:true});
    if (Array.isArray(turns)) agent.turns = turns;
  } catch { /* the next send reports the real error */ }
  renderLog();
  await acknowledgeAgentReply();
}

// A reply receipt is emitted only after this project's actual assistant node is displayed.
async function acknowledgeAgentReply(revision) {
  const project = state.project;
  const turn = [...agent.turns].reverse().find((t) => t.role === "assistant" && t.request_id && t.project === project);
  if (!agent.open || !turn) return;
  await new Promise(requestAnimationFrame);
  if (!agent.open || state.project !== project) return;
  const visible = [...$("agent-log").querySelectorAll(".msg.assistant")].some((n) => n.textContent === turn.text && n.getBoundingClientRect().width > 0 && n.getBoundingClientRect().height > 0);
  if (!visible) return;
  if (revision) await acFeedback.acknowledge(revision, project);
  const key = `${project}:${turn.request_id}:${turn.session}`;
  if (agent.renderedReply === key) return;
  await acFeedback.report(null, null, {stage:"frontend_ack",command:"agent.send",request_id:turn.request_id,session:turn.session,reply_rendered:true,ui_ack:"assistant reply displayed; visual review separate"});
  agent.renderedReply = key;
}

function turnItems() {
  const items = [];
  if (!agent.turns.length) {
    items.push({ key: "notice:empty", cls: "msg done",
      text: "Ask the agent anything about this project. Point at something to show it what you mean." });
  }
  agent.turns.forEach((turn, i) => {
    const key = `t:${i}:${turn.at || 0}`;
    if (turn.role === "user") items.push({ key, cls: "msg user", text: turn.text || "" });
    else if (turn.role === "assistant") items.push({ key, cls: "msg assistant", text: turn.text || "" });
    else if (turn.role === "error") items.push({ key, cls: "msg tool", text: `· ${turn.text || "failed"}` });
    if (turn.role === "assistant" && turn.tools?.length) {
      items.push({ key: `${key}:tools`, cls: "msg tool", text: `· ${turn.tools.join(", ")}` });
    }
  });
  return items;
}

function renderLog() {
  const log = $("agent-log");
  const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 40;
  const top = log.scrollTop;

  const existing = new Map();
  for (const child of log.children) if (child.dataset?.k) existing.set(child.dataset.k, child);

  const next = turnItems().map((item) => {
    const node = existing.get(item.key);
    if (!node) {
      const made = el("div", { class: item.cls }, item.text);
      made.dataset.k = item.key;
      return made;
    }
    existing.delete(item.key);
    if (node.textContent !== item.text) node.textContent = item.text;
    return node;
  });

  for (const gone of existing.values()) gone.remove();
  for (let i = 0; i < next.length; i++) {
    const node = next[i];
    if (node.parentNode === log) continue;
    let anchor = null;
    for (let j = i + 1; j < next.length; j++) if (next[j].parentNode === log) { anchor = next[j]; break; }
    log.insertBefore(node, anchor);
  }

  if (atBottom) log.scrollTop = log.scrollHeight;
  else log.scrollTop = top;
}

function renderAttachments() {
  $("agent-attachments").replaceChildren(...agent.attachments.map((a, i) =>
    el("div", { class: "att", title: a.path },
      el("img", { src: convertFileSrc(a.path), alt: "" }), el("span", {}, a.what),
      el("button", { "aria-label": "Remove", title: "Remove", onclick: () => { agent.attachments.splice(i, 1); renderAttachments(); } }, "×"))));
}

// The draft and its pictures are handed over only once the turn is delivered: a failed send keeps
// them, and anything typed or attached while it ran stays in the composer.
function clearSent(input, raw, attachments) {
  if (input.value === raw) input.value = "";
  const sent = new Set(attachments);
  if (agent.attachments.some((a) => sent.has(a))) {
    agent.attachments = agent.attachments.filter((a) => !sent.has(a));
    renderAttachments();
  }
}

async function agentNewChat() {
  if (agent.sending) throw new Error("Wait for the current message to finish");
  const button = $("agent-new"), project = state.project;
  button.disabled = true;
  try {
    const reply = await motionCommand("agent.new_chat");
    if (state.project !== project) return;
    agent.turns = Array.isArray(reply.turns) ? reply.turns : [];
    agent.newChat = !reply.session;
    renderLog();
    status(`New chat ready${reply.session ? ` · ${reply.session}` : ""} · previous conversation saved`);
    $("agent-input").focus();
    if (reply._ui_engine_revision) await acFeedback.acknowledge(reply._ui_engine_revision, project);
  } finally { button.disabled = false; }
}
$("agent-new").addEventListener("click", () => agentNewChat().catch((e) => status(String(e))));

async function agentSend() {
  const input = $("agent-input");
  if (agent.sending) return;
  const raw = input.value;
  const text = raw.trim();
  if (!text && !agent.attachments.length) return;
  const attachments = agent.attachments.slice();
  agent.sending = true;
  $("agent-send").disabled = true;
  status("Sending…");
  try {
    const context = {
      project: state.project, view: state.view, storyboard_page: state.page + 1,
      deck: state.data?.storyboard?.deck,
      animation: { asset: motion.character, track: motion.track, time: motion.time },
    };
    const message = `${text}\n\n[Magic Studio context at send time: ${JSON.stringify(context)}]`;
    const reply = await motionCommand("agent.send", {
      message,
      new_chat: agent.newChat,
      attachments: attachments.map((a) => a.path),
    });
    agent.newChat = true;
    if (Array.isArray(reply.turns)) agent.turns = reply.turns;
    renderLog();
    await acknowledgeAgentReply(reply._ui_engine_revision);
    clearSent(input, raw, attachments);
    status(reply.ok ? `Answered${reply.provider ? ` · ${reply.provider}` : ""}${reply.model ? ` / ${reply.model}` : " · CLI default unverified"} · ${reply.session || "fresh process"}` : String(reply.error || "Agent needs attention in its existing session; do not resend blindly."));
  } catch (error) {
    status(String(error));
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
  overlay.classList.add("picking");
  const node = document.elementFromPoint(x, y);
  overlay.classList.remove("picking");
  return node && !node.closest("#agent") ? node : null;
}

function pointStart() {
  const overlay = $("point-overlay"), box = $("point-box");
  overlay.classList.add("on");
  const previousDrawer=workshopDrawers.active;workshopDrawers.close(false);
  let target = null;
  const move = (e) => {
    target = pick(e.clientX, e.clientY);
    if (!target) { box.style.setProperty("--point-width", "0px"); return; }
    const r = target.getBoundingClientRect();
    for (const [key,value] of Object.entries({left:r.left-3,top:r.top-3,width:r.width+6,height:r.height+6})) box.style.setProperty(`--point-${key}`,`${value}px`);
  };
  const stop = (resume=true) => {
    overlay.classList.remove("on");
    overlay.removeEventListener("mousemove", move);
    overlay.removeEventListener("click", click);
    document.removeEventListener("keydown", esc, true);
    box.style.setProperty("--point-width", "0px");
    if(resume&&previousDrawer)workshopDrawers.open(previousDrawer);
  };
  const click = async (e) => {
    e.preventDefault();
    const node = target || pick(e.clientX, e.clientY);
    stop(false);
    if (!node) {if(previousDrawer)workshopDrawers.open(previousDrawer);return;}
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
    finally{if(previousDrawer)workshopDrawers.open(previousDrawer);}
  };
  const esc = (e) => { if (e.key === "Escape") { e.stopPropagation(); stop(); } };
  overlay.addEventListener("mousemove", move);
  overlay.addEventListener("click", click);
  document.addEventListener("keydown", esc, true);
}

const workshopDrawers=createTopDrawers({
  canOpen:()=>{if(state.project)return true;toast("Open a project first");return false;},
  onLayout:height=>invoke("drawer_layout",{height}).catch(e=>status(String(e))),
  onSettings:id=>{if(id==='anim-agent-settings')refreshModels();},
  onChange:name=>{
    agent.open=name==='sproket';
    $("agent").setAttribute("aria-hidden",String(!agent.open));
    window.magicCompanions?.sync();clearInterval(agent.timer);
    if(agent.open){
      if(agent.loaded)agentReloadLog();else agentLoad();
      agent.timer=setInterval(()=>{if(!agent.sending)agentReloadLog();},1500);
    }
    if(name==='sproket'||name==='gaffer')$(name==='sproket'?'agent-input':'image-prompt')?.focus();
  },
});
window.workshopDrawers=workshopDrawers;
function agentToggle(open=!agent.open){
  if(open)workshopDrawers.open('sproket');
  else if(workshopDrawers.active==='sproket')workshopDrawers.close();
}

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
$("agent-send").addEventListener("click", agentSend);
$("agent-point").addEventListener("click", pointStart);
$("agent-catalog-refresh").addEventListener("click", refreshModels);
$("agent-command").addEventListener("change", refreshModels);
$("agent-provider").addEventListener("input", renderModelOptions);
$("agent-config-save").addEventListener("click", configSave);
$("agent-input").addEventListener("keydown", (e) => {
  // An IME candidate is not a send: Enter only picks the candidate.
  if (e.isComposing || e.key === "Process") return;
  if (e.key !== "Enter") return;
  if (e.shiftKey) return; // Shift+Enter is a line break.
  e.preventDefault();
  agentSend();
});
document.addEventListener("keydown", (e) => {
  if ((e.key === "/" || e.key === "\\") && (e.ctrlKey || e.metaKey)) { e.preventDefault(); agentToggle(); return; }
});
