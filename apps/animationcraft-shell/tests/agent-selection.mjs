// Real drawer handlers in a minimal DOM fixture. No browser, agent process, credentials or network.
import vm from "node:vm";
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const nodes = new Map(), calls = [];
const node = (id) => {
  if (!nodes.has(id)) nodes.set(id, {
    value: "", hidden: true, disabled: false, textContent: "", title: "", dataset: {}, children: [],
    querySelectorAll() { return []; }, addEventListener() {}, focus() {}, replaceChildren(...children) { this.children = children; },
  });
  return nodes.get(id);
};
let rejectSend = false;
const config = { agent: "opencode", provider: "opencode", model: "synthetic-free", agents: [{ name: "opencode", installed: true, headless: true }] };
const context = vm.createContext({
  console, JSON, Promise, Set, $: node,
  requestAnimationFrame: (callback) => callback(),
  acFeedback: { acknowledge: async (revision, project) => calls.push({id:"render_revision",revision,project}), report: async (phase, ack, event) => calls.push({id:"render_receipt",event}) },
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

// A reply acknowledgement requires the matching project and an actually displayed assistant node.
node("agent-log").querySelectorAll = () => [{textContent:"Visible reply",getBoundingClientRect:()=>({width:300,height:20})}];
await vm.runInContext(`agent.open=true; agent.turns=[{role:"assistant",text:"Visible reply",project:"/other.acproj",request_id:"other",session:"B"}]; acknowledgeAgentReply(10)`,context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,0,"other project cannot acknowledge");
await vm.runInContext(`agent.turns=[{role:"assistant",text:"Not rendered",project:state.project,request_id:"r1",session:"A"}]; acknowledgeAgentReply(10)`,context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,0,"unrendered text cannot acknowledge");
await vm.runInContext(`agent.turns=[{role:"assistant",text:"Visible reply",project:state.project,request_id:"r1",session:"A"}]; acknowledgeAgentReply(10)`,context);
assert.equal(calls.filter(c=>c.id==="render_revision").length,1);
assert.equal(calls.find(c=>c.id==="render_receipt").event.request_id,"r1");
await vm.runInContext("acknowledgeAgentReply()",context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,1,"polling must not duplicate receipt");
vm.runInContext("agent.open=false; agent.renderedReply=null",context);
await vm.runInContext("acknowledgeAgentReply(11)",context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,1,"closed drawer cannot acknowledge");
console.log("PASS: reply receipts require actual visible text, project match, open drawer and correlated request; repeated polling is idempotent");
