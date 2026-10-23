// Runs the REAL apps/magicstudio-shell/ui/agent.js in Chromium against a harness page and
// asserts the agent-drawer behaviour: no fade replay on unchanged content, node reuse, scroll,
// selection, in-progress question input, model options, and the send/draft rules.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || "playwright");
import { readFileSync } from "node:fs";

const UI = new URL("../ui/", import.meta.url).pathname;
const agentJs = readFileSync(process.env.AGENT_JS || `${UI}/agent.js`, "utf8");
const css = readFileSync(`${UI}/style.css`, "utf8");
const harness = readFileSync(new URL("agent-harness.html", import.meta.url), "utf8");

const results = [];
const check = (name, ok, detail = "") => { results.push({ name, ok, detail }); };

const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || "/usr/bin/chromium" });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
page.on("pageerror", (e) => check("no page error", false, String(e)));
await page.setContent(harness);
await page.addStyleTag({ content: css + "\n.agent{transform:none !important}" });
await page.addScriptTag({ content: `
  const $ = (id) => document.getElementById(id);
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
  const motion = {character:null,track:null,time:0};
  const state = { project: "/tmp/proj/Test.magic", view: "project" };
  const toast = () => {};
  const convertFileSrc = (p) => p;
  window.__calls = [];
  window.__failPrompt = false;
  window.__promptGate = null;
  window.__invoke = async (cmd, args) => {
    const a = (args && args.args) || [];
    window.__calls.push([cmd, ...a]);
    if (cmd === "opencode") {
      if (a[0] === "session.message.list") return { data: window.__messages.slice().reverse() };
      if (a[0] === "session.permission.list") return { data: window.__permissions };
      if (a[0] === "session.form.list") return { data: window.__forms };
      if (a[0] === "model.list") return { data: [{ providerID: "opencode-go", id: "m1", name: "Model One" }] };
      if (a[0] === "session.prompt") {
        if (window.__promptGate) await window.__promptGate;
        if (window.__failPrompt) throw new Error("opencode: service unavailable");
        return { data: { id: "msg_sent" } };
      }
      return { data: null };
    }
    if (cmd === "herdr") return { result: { agents: [] } };
    if (cmd === "agent_session") return "ses_test";
    return null;
  };
  const invoke = (cmd, args) => window.__invoke(cmd, args);
  window.__messages = []; window.__permissions = []; window.__forms = [];
` });
await page.addScriptTag({ content: agentJs });

// --- shared setup ---------------------------------------------------------
await page.evaluate(async () => {
  window.__fades = 0;
  document.addEventListener("animationstart", (e) => {
    if (e.animationName === "fade" && e.target.classList && e.target.classList.contains("msg")) {
      window.__fades++;
      (window.__fadeLog ||= []).push({ text: (e.target.textContent || "").slice(0, 14), isNew: !window.__known?.has(e.target) });
    }
  }, true);
  const msg = (id, role, text) => ({ id, type: role === "user" ? "user" : "assistant", role, text,
    content: role === "user" ? undefined : [{ type: "text", text }] });
  window.__messages = [msg("m1", "user", "first question"), msg("m2", "assistant", "first answer")];
  agent.open = true;
  agent.session = "ses_test";
  agent.models = [{ providerID: "opencode-go", id: "m1", name: "Model One" }];
  await agentPoll();
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
});

// --- 1. unchanged content: same nodes, no fade replay ---------------------
const s1 = await page.evaluate(async () => {
  const log = document.getElementById("agent-log");
  const before = [...log.children];
  window.__known = new Set(before);
  const fades = window.__fades;
  await agentPoll(); await agentPoll(); await agentPoll();
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  return { same: before.every((n) => [...log.children].includes(n)), n: [...log.children].length,
    newFades: window.__fades - fades, firstSame: log.firstElementChild === before[0] };
});
check("unchanged content keeps every node", s1.same, JSON.stringify(s1));
check("unchanged content replays no fade", s1.newFades === 0, `newFades=${s1.newFades}`);
check("unchanged content keeps the first node", s1.firstSame, "");

// --- 2. new content: old nodes kept, exactly one new fade ----------------
const s2 = await page.evaluate(async () => {
  const log = document.getElementById("agent-log");
  const before = [...log.children];
  const fades = window.__fades;
  window.__messages.push({ id: "m3", type: "assistant", role: "assistant", content: [{ type: "text", text: "second answer" }] });
  await agentPoll();
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  const kept = before.every((n) => [...log.children].includes(n));
  const added = [...log.children].length - before.length;
  return { kept, added, newFades: window.__fades - fades, fadesAtStart: fades, fadeLog: window.__fadeLog, order: [...log.children].map((n) => n.textContent.slice(0, 20)) };
});
check("new content keeps the old nodes", s2.kept, "");
check("new content adds one node", s2.added === 1, `added=${s2.added}`);
check("new content fades only the new node", s2.newFades === 1, `newFades=${s2.newFades}`);
check("new content keeps the order", s2.order[0].startsWith("first questi") && s2.order[2].startsWith("second answer"), JSON.stringify(s2.order));

// --- 3. text selection survives a poll ------------------------------------
const s3 = await page.evaluate(async () => {
  const log = document.getElementById("agent-log");
  const first = log.firstElementChild;
  const range = document.createRange();
  range.setStart(first.firstChild, 0); range.setEnd(first.firstChild, 5);
  const sel = getSelection(); sel.removeAllRanges(); sel.addRange(range);
  const before = sel.toString();
  await agentPoll();
  return { before, after: getSelection().toString() };
});
check("text selection survives a poll", s3.before.length > 0 && s3.after === s3.before, JSON.stringify(s3));

// --- 4. scroll position survives a poll -----------------------------------
const s4 = await page.evaluate(async () => {
  const log = document.getElementById("agent-log");
  for (let i = 0; i < 30; i++) window.__messages.push({ id: `f${i}`, type: "assistant", content: [{ type: "text", text: "filler ".repeat(40) }] });
  await agentPoll();
  log.scrollTop = 120;
  const before = log.scrollTop;
  await agentPoll();
  return { before, after: log.scrollTop, scrollable: log.scrollHeight > log.clientHeight };
});
check("scroll position survives a poll", s4.scrollable && s4.after === s4.before, JSON.stringify(s4));

// --- 5. an answer being typed into a question card survives ---------------
const s5 = await page.evaluate(async () => {
  window.__forms = [{ id: "f1", sessionID: "ses_test", fields: [{ key: "answer", title: "Question", custom: true }] }];
  await agentPoll();
  const input = document.querySelector("#agent-log .card input");
  input.value = "my typed answer";
  const node = input;
  await agentPoll();
  const now = document.querySelector("#agent-log .card input");
  return { sameNode: now === node, value: now && now.value, cards: document.querySelectorAll("#agent-log .card").length };
});
check("question input node is reused", s5.sameNode, JSON.stringify(s5));
check("typed answer survives a poll", s5.value === "my typed answer", `value=${s5.value}`);

// --- 6. model options are not rebuilt --------------------------------------
const s6 = await page.evaluate(async () => {
  const sel = document.getElementById("agent-model");
  const opt0 = sel.options[0];
  await agentPoll();
  return { same: sel.options[0] === opt0, n: sel.options.length, value: sel.value };
});
check("model options are not rebuilt", s6.same, JSON.stringify(s6));

// --- 7. a failing send keeps the draft and the pictures -------------------
const s7 = await page.evaluate(async () => {
  window.__failPrompt = true;
  const input = document.getElementById("agent-input");
  input.value = "keep this draft";
  agent.attachments = [{ path: "/tmp/a.png", what: "a picture" }];
  renderAttachments();
  await agentSend();
  return { draft: input.value, attachments: agent.attachments.length,
    status: document.getElementById("agent-status").textContent };
});
check("failed send keeps the draft", s7.draft === "keep this draft", `draft=${JSON.stringify(s7.draft)}`);
check("failed send keeps the attachments", s7.attachments === 1, `attachments=${s7.attachments}`);
check("failed send shows the error", /unavailable/.test(s7.status), `status=${s7.status}`);

// --- 8. a successful send clears what was sent ----------------------------
const s8 = await page.evaluate(async () => {
  window.__failPrompt = false;
  const input = document.getElementById("agent-input");
  input.value = "send me";
  agent.attachments = [{ path: "/tmp/a.png", what: "a picture" }];
  await agentSend();
  return { draft: input.value, attachments: agent.attachments.length };
});
check("successful send clears the draft", s8.draft === "", `draft=${JSON.stringify(s8.draft)}`);
check("successful send removes the sent picture", s8.attachments === 0, `attachments=${s8.attachments}`);

// --- 9. text typed while the send is pending survives ---------------------
const s9 = await page.evaluate(async () => {
  let release;
  window.__promptGate = new Promise((r) => { release = r; });
  const input = document.getElementById("agent-input");
  input.value = "first part";
  const sending = agentSend();
  await new Promise((r) => setTimeout(r, 30));
  input.value = "first part and more";
  release();
  await sending;
  return { draft: input.value, sending: agent.sending };
});
check("text typed while pending survives", s9.draft === "first part and more", `draft=${JSON.stringify(s9.draft)}`);
check("sending flag is cleared", s9.sending === false, "");

// --- 10. a second send while one is pending is ignored -------------------
const s10 = await page.evaluate(async () => {
  let release;
  window.__promptGate = new Promise((r) => { release = r; });
  const before = window.__calls.filter((c) => c[1] === "session.prompt").length;
  const input = document.getElementById("agent-input");
  input.value = "one";
  const a = agentSend();
  await new Promise((r) => setTimeout(r, 20));
  const b = agentSend();
  release();
  await a; await b;
  const after = window.__calls.filter((c) => c[1] === "session.prompt").length;
  window.__promptGate = null;
  return { sent: after - before, disabled: document.getElementById("agent-send").disabled };
});
check("a second send while pending is ignored", s10.sent === 1, `prompts=${s10.sent}`);
check("the send button is re-enabled", s10.disabled === false, "");

// --- 11. Enter sends, Shift+Enter is a line break, IME is ignored ---------
const s11 = await page.evaluate(async () => {
  const input = document.getElementById("agent-input");
  const key = (init) => {
    const ev = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true, ...init });
    input.dispatchEvent(ev);
    return ev.defaultPrevented;
  };
  input.value = "enter test";
  const before = window.__calls.filter((c) => c[1] === "session.prompt").length;
  const preventedEnter = key({});
  await new Promise((r) => setTimeout(r, 60));
  const afterEnter = window.__calls.filter((c) => c[1] === "session.prompt").length;
  const preventedShift = key({ shiftKey: true });
  const preventedIme = key({ isComposing: true });
  input.value = "ctrl enter test";
  const preventedCtrl = key({ ctrlKey: true });
  await new Promise((r) => setTimeout(r, 60));
  const afterCtrl = window.__calls.filter((c) => c[1] === "session.prompt").length;
  return { preventedEnter, preventedShift, preventedIme, preventedCtrl, enterSent: afterEnter - before, ctrlSent: afterCtrl - afterEnter };
});
check("Enter sends", s11.enterSent === 1 && s11.preventedEnter, JSON.stringify(s11));
check("Shift+Enter is a line break", s11.preventedShift === false, "");
check("an IME candidate does not send", s11.preventedIme === false, "");
check("Ctrl+Enter still sends", s11.ctrlSent === 1 && s11.preventedCtrl, JSON.stringify(s11));

await browser.close();

let failed = 0;
for (const r of results) {
  if (!r.ok) failed++;
  console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name}${r.detail ? "   " + r.detail : ""}`);
}
console.log(`\n${results.length - failed}/${results.length} passed`);
process.exit(failed ? 1 : 0);
