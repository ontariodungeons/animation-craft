// Real drawer handlers in a minimal DOM fixture. No browser, agent process, credentials or network.
import vm from "node:vm";
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const nodes = new Map(), calls = [];
const node = (id) => {
  if (!nodes.has(id)) nodes.set(id, {
    value: "", hidden: true, disabled: false, textContent: "", title: "", dataset: {}, children: [],
    addEventListener() {}, focus() {}, replaceChildren(...children) { this.children = children; },
  });
  return nodes.get(id);
};
let rejectSend = false;
const config = { agent: "opencode", provider: "opencode", model: "synthetic-free", agents: [{ name: "opencode", installed: true, headless: true }] };
const context = vm.createContext({
  console, JSON, Promise, Set, $: node,
  el: (tag, attrs, text) => ({ tag, ...attrs, text }),
  document: { addEventListener() {} }, window: { addEventListener() {} },
  state: { project: "/synthetic/Test.acproj", view: "project", page: 0 }, motion: {},
  invoke: async (id, { patch }) => { calls.push({ id, patch }); return { ...config, ...patch }; },
  motionCommand: async (id, params) => {
    calls.push({ id, params });
    if (id === "agent.models") return { available: true, models: [{ providerID: "opencode", id: "synthetic-free", enabled: true }] };
    if (id === "agent.new_chat") return { ok: true, session: "ses_fresh", turns: [] };
    if (id === "agent.send") {
      if (rejectSend) throw Error("Synthetic authentication rejected");
      return { ok: true, session: "ses_fresh", provider: "opencode", model: "synthetic-free", text: "Synthetic answer", turns: [] };
    }
    throw Error(`Unexpected operation ${id}`);
  },
});
vm.runInContext(readFileSync(new URL("../ui/agent.js", import.meta.url), "utf8"), context);
// Rendering is separate from the selection and send handlers under test.
vm.runInContext("renderLog=()=>{}; renderAttachments=()=>{}; agent.config=" + JSON.stringify(config) + "; renderConfig();", context);
assert.equal(node("agent-provider").defaultValue, node("agent-provider").value, "loaded provider is not an unsaved draft");
assert.equal(node("agent-model").defaultValue, node("agent-model").value, "loaded model is not an unsaved draft");
await vm.runInContext("refreshModels()", context);
assert.equal(node("agent-providers").children[0].value, "opencode");
assert.equal(node("agent-models").children[0].value, "synthetic-free");
await vm.runInContext("configSave()", context);
const saved = calls.find((c) => c.id === "agent_config_save");
assert.equal(saved.patch.provider, "opencode");
assert.equal(saved.patch.model, "synthetic-free");
assert.ok(!("image_model" in saved.patch), "chat save must not overwrite image preferences");
await vm.runInContext("agentNewChat()", context);
node("agent-input").value = "Synthetic first message";
await vm.runInContext("agentSend()", context);
let sends = calls.filter((c) => c.id === "agent.send");
assert.equal(sends[0].params.new_chat, false, "use the explicitly prepared fresh session once");
assert.ok(!("model" in sends[0].params), "backend must read shared app selection");
assert.ok(node("agent-status").textContent.includes("opencode / synthetic-free"));
node("agent-input").value = "Synthetic second message";
await vm.runInContext("agentSend()", context);
sends = calls.filter((c) => c.id === "agent.send");
assert.equal(sends[1].params.new_chat, true, "later messages must not silently resume");
rejectSend = true;
node("agent-input").value = "Keep failed draft";
await vm.runInContext("agentSend()", context);
assert.equal(node("agent-input").value, "Keep failed draft");
assert.ok(node("agent-status").textContent.includes("authentication rejected"));
console.log("PASS: actual drawer catalog, saved selection, New Chat, fresh send, model reporting and failed-draft handlers");
