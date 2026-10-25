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
  createTopDrawers:()=>({open(){},close(){},active:null}),
  requestAnimationFrame: (callback) => callback(),
  acFeedback: { acknowledge: async (revision, project) => calls.push({id:"render_revision",revision,project}), report: async (phase, ack, event) => calls.push({id:"render_receipt",event}) },
  el: (tag, attrs, text) => ({ tag, ...attrs, text }),
  document: { addEventListener() {} }, window: { addEventListener() {} },
  state: { project: "/synthetic/Test.magic", view: "project", page: 0 }, motion: {},
  invoke: async (id, { patch } = {}) => { if (id === "agent_session") throw Error("Obsolete agent_session command is not registered"); calls.push({ id, patch }); return { ...config, ...patch }; },
  motionCommand: async (id, params) => {
    calls.push({ id, params });
    if (id === "agent.log") return [];
    if (id === "agent.models") return { available: true, models: [{ providerID: "opencode", id: "synthetic-free", enabled: true }] };
    if (id === "agent.new_chat") return { ok: true, session: "ses_fresh", turns: [], _ui_engine_revision: 2 };
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
assert.ok(calls.some(c => c.id === "render_revision" && c.revision === 2 && c.project === "/synthetic/Test.magic"), "New Chat acknowledges its applied UI revision");
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
await vm.runInContext(`agent.open=true; agent.turns=[{role:"assistant",text:"Visible reply",project:"/other.magic",request_id:"other",session:"B"}]; acknowledgeAgentReply(10)`,context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,0,"other project cannot acknowledge");
await vm.runInContext(`agent.turns=[{role:"assistant",text:"Not rendered",project:state.project,request_id:"r1",session:"A"}]; acknowledgeAgentReply(10)`,context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,0,"unrendered text cannot acknowledge");
await vm.runInContext(`agent.turns=[{role:"assistant",text:"Visible reply",project:state.project,request_id:"r1",session:"A"}]; acknowledgeAgentReply(10)`,context);
assert.equal(calls.filter(c=>c.id==="render_revision" && c.revision===10).length,1);
assert.equal(calls.find(c=>c.id==="render_receipt").event.request_id,"r1");
await vm.runInContext("acknowledgeAgentReply()",context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,1,"polling must not duplicate receipt");
vm.runInContext("agent.open=false; agent.renderedReply=null",context);
await vm.runInContext("acknowledgeAgentReply(11)",context);
assert.equal(calls.filter(c=>c.id==="render_receipt").length,1,"closed drawer cannot acknowledge");
console.log("PASS: reply receipts require actual visible text, project match, open drawer and correlated request; repeated polling is idempotent");

// CLI New Chat refresh uses the same drawer loader, not removed Tauri chat commands.
vm.runInContext(readFileSync(new URL("../ui/bridge.js", import.meta.url), "utf8"), context);
await vm.runInContext(`agent.turns=[{role:"assistant",text:"Previous conversation"}]; window.magicBridge.refresh("agent.new_chat",state.project,12,{session:"ses_fresh",turns:[]})`,context);
assert.equal(vm.runInContext("agent.turns.length",context),0);
assert.equal(vm.runInContext("agent.newChat",context),false);
assert.ok(calls.some(c=>c.id==="render_revision"&&c.revision===12));
console.log("PASS: bridged New Chat reloads shared drawer state and acknowledges its revision");

// The shared drawer must remain available when Project is hidden for Animation/Studio.
const shell = readFileSync(new URL("../ui/index.html", import.meta.url), "utf8");
const projectStart = shell.indexOf('id="view-project"');
const animationStart = shell.indexOf('id="view-animation"');
const projectEnd = shell.lastIndexOf('</section>', animationStart);
const drawer = shell.indexOf('id="agent"');
assert.ok(drawer > projectEnd && drawer > projectStart, "shared drawer cannot be nested in Project");
assert.ok(shell.indexOf('</main>') < drawer, "drawer is independent of tab-specific view visibility");
console.log("PASS: shared Sproket drawer is outside tab-specific containers");

// Supplemental Blender rendering cannot replace the authoring editor or its part selection.
// Run real mode-switch, selection-label and Gaffer handlers; only native transport is synthetic.
const artworkNodes = new Map(), artworkCalls = [];
function artworkElement(tag, attrs = {}, ...children) {
  const n = { tag, ...attrs, _value:attrs.value, children: [], parentElement:null, style: {}, dataset: {}, events:{}, textContent: children.filter(c => typeof c === 'string').join(''),
    classList: { add() {} }, addEventListener(type,fn) {this.events[type]=fn;}, dispatchEvent() {}, focus() {}, pause() {}, load() {}, removeAttribute() {}, setAttribute(name,value) {this[name]=value;}, replaceWith() {}, showModal(){this.open=true;},close(){this.open=false;this.events.close?.();},remove(){},
    append(...items) { for(const item of items.flat()){if(item&&typeof item==='object'){if(item.parentElement){const previous=item.parentElement;previous.children.splice(previous.children.indexOf(item),1);}item.parentElement=this;}this.children.push(item);} },
    replaceChildren(...items) { for(const child of this.children)if(child&&typeof child==='object')child.parentElement=null;this.children=[];this.append(...items); },
    insertBefore(item,reference) { if(reference!==null&&!this.children.includes(reference))throw new DOMException('Reference is not a child','NotFoundError');if(item===reference)return item;if(item.parentElement){const previous=item.parentElement;previous.children.splice(previous.children.indexOf(item),1);}this.children.splice(reference===null?this.children.length:this.children.indexOf(reference),0,item);item.parentElement=this;return item; },
    querySelector(selector) {
      const cls = selector.split(' ').at(-1).slice(1);
      const find = node => node && typeof node === 'object' && ((node.class || '').split(' ').includes(cls) ? node : node.children?.map(find).find(Boolean));
      return this.children.map(find).find(Boolean) || null;
    },
    get options() { return this.children.filter(c => c?.tag === 'option'); },
    get selectedOptions() { return this.options.filter(o => o.value === this.value); },
    get value() { return this._value ?? this.options[0]?.value ?? ''; }, set value(v) { this._value = v; },
  };
  n.append(...children.flat());
  if (attrs.id) artworkNodes.set(attrs.id, n);
  return n;
}
// DOM insertion moves an existing node, preserves parent links, and rejects a foreign reference.
{
 const a=artworkElement('a'),b=artworkElement('b'),parent=artworkElement('div',{},a,b),other=artworkElement('div');
 assert.equal(parent.insertBefore(b,a),b);assert.deepEqual(parent.children,[b,a]);assert.equal(b.parentElement,parent);
 other.insertBefore(b,null);assert.deepEqual(parent.children,[a]);assert.equal(b.parentElement,other);
 assert.throws(()=>parent.insertBefore(b,b),{name:'NotFoundError'});
 parent.replaceChildren(b);assert.equal(a.parentElement,null);assert.equal(b.parentElement,parent);assert.equal(other.children.length,0);
}
for (const id of ['view-animation', 'magic-target-image', 'magic-target-parts']) artworkElement('div', { id });
const artworkContext = vm.createContext({
  console, Map, Set, URL, Promise, queueMicrotask, performance, Event, clearTimeout, setTimeout, clearInterval, setInterval,
  localStorage: { getItem: () => null, setItem() {} },
  state: { project: '/fixture/Artwork.magic', view: 'animation', open: 'image' },
  $: id => artworkNodes.get(id) || null, el: artworkElement, ICONS: { chev: '' }, convertFileSrc: p => p,
  document: { querySelectorAll: () => [], querySelector: () => null, body:{append(){}}, createElement: tag => artworkElement(tag) }, window: { addEventListener() {} },
  magicTheatre: { exports: new Map() }, theatreRequireSaved() {}, theatreCanLeave: () => true,
  updateAppFooter() {}, renderBlocks() {}, acFeedback: { report: async () => {} },
  invoke: async (id, args) => {
    artworkCalls.push({ id, ...args });
    if (id !== 'engine_command') throw Error(`Unexpected fixture transport ${id}`);
    if (args.id === 'project.list_assets') return { assets: [{ id: 'paper-puppet', kind: 'cutout_rig', name: 'Paper puppet' }] };
    if (args.id === 'blender.actions') return { character: 'paper-puppet', tracks: [{ id: 'wave', duration: 1, standard: false }], layers: [], canvas: [64, 64], spec: '/fixture/rig.json' };
    if (args.id === 'blender.list') return { jobs: [] };
    if (args.id === 'storyboard.toc') return { chapters: [] };
    if (args.id === 'image.generate') throw Error('Native generation disabled in fixture');
    throw Error(`Unexpected fixture command ${args.id}`);
  },
});
for (const file of ['companion-boxes.js', 'animation.js', 'blender-placement.js', 'blender.js', 'image-prompt.js']) {
  vm.runInContext(readFileSync(new URL(`../ui/${file}`, import.meta.url), 'utf8'), artworkContext);
}
const appSource = readFileSync(new URL('../ui/app.js', import.meta.url), 'utf8');
// Use the production shared-label and block renderers without booting the desktop app.
for (const [start, end] of [['function updateMagicSelection()', '\nfunction projectAssetGrid'], ['function block(', '\nfunction objectsFrom(']]) {
  vm.runInContext(appSource.slice(appSource.indexOf(start), appSource.indexOf(end)), artworkContext);
}
vm.runInContext(`
  motion.mode='subject'; motion.character='paper-puppet'; motion.selected='left-piece';
  motion.meta={original_reference:'/fixture/source.png',layers:[
    {id:'left-piece',label:'Left piece',file:'/fixture/left.png'},
    {id:'right-piece',label:'Right piece',file:'/fixture/right.png'}]};
  updateMagicSelection();
  const captured=selectedPartTarget();
  const existingResult={asset_id:'earlier-candidate',image:'/fixture/candidate.png'};
  const existingDraft={prompt:'Draw a whole new artwork',model:'gpt-6-luna',busy:false,error:'',target:captured,candidateTarget:captured,result:existingResult};
  imageDrafts.set(state.project,existingDraft);
`, artworkContext);
assert.match(artworkNodes.get('magic-target-image').textContent, /Left piece/);
assert.equal(vm.runInContext('selectedPartTarget().partId', artworkContext), 'left-piece');
// Click the actual Blender mode control, including its normal metadata loading.
await vm.runInContext(`motionModeSwitch().children.find(n=>n.id==='motion-mode-blender').onclick()`, artworkContext);
assert.equal(vm.runInContext('motion.selected', artworkContext), 'left-piece', 'rendering must preserve the authoring part ID');
assert.equal(artworkNodes.get('blender-render-dialog').open,true,'actual render dialog must open');
assert.equal(artworkNodes.get('magic-target-image').textContent, 'Left piece · left-piece');
assert.equal(vm.runInContext('selectedPartTarget().partId', artworkContext), 'left-piece');
assert.equal(vm.runInContext('motion.meta.layers.length', artworkContext), 2, 'lightweight render metadata must not replace authoring layers');
assert.ok(artworkCalls.some(c=>c.id==='blender.actions'),'fixture must reach the real render metadata query');
assert.equal(vm.runInContext('existingDraft.target===captured && existingDraft.candidateTarget===captured && existingDraft.result===existingResult', artworkContext), true, 'switching backend preserves captured generation data');
assert.equal(vm.runInContext('JSON.stringify(captured.references)', artworkContext), '["source.png","left.png"]');
await vm.runInContext(`imagePromptBlock();`, artworkContext);
await artworkNodes.get('image-generate').onclick();
const wholeArtwork = artworkCalls.find(c => c.id === 'image.generate');
assert.ok(wholeArtwork, 'valid captured part request still reaches the transport');
assert.equal(wholeArtwork.params.prompt, 'Draw a whole new artwork');
assert.equal(JSON.stringify(wholeArtwork.params.references),'["source.png","left.png"]','render selection must not retarget generation references');
assert.equal(vm.runInContext('existingDraft.error', artworkContext), 'Error: Native generation disabled in fixture');
console.log('PASS: supplemental Blender controls preserve authoring selection and captured generation references');
await import('./companions.mjs');
await import('./animation-authoring.mjs');

await import('./inspector-controls.mjs');

await import('./blender-placement.mjs');

await import('./animation-refresh.mjs');

await import('./animation-bridge.mjs');

await import('./animation-transport.mjs');

await import('./animation-native.mjs');

await import('./motion-authoring-controls.mjs');

await import('./animation-session.mjs');

await import('./animation-feedback.mjs');
