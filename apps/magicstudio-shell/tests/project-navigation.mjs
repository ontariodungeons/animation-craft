// Exercise the real New form and project rendering handlers without a live Studio.
import vm from 'node:vm';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const nodes = new Map(), calls = [], toasts = [];
function node(id) {
  if (!nodes.has(id)) nodes.set(id, {value:'',textContent:'',disabled:false,handlers:{},classList:{toggle(){},contains(){return false;}},setAttribute(){},focus(){},prepend(...items){this.items=items;},reset(){calls.push('reset');},addEventListener(type,fn){this.handlers[type]=fn;}});
  return nodes.get(id);
}
const result={ok:true,project:'/chosen/ab/project/A&B.magic',intake_ready:false,problems:['No valid transcript yet'],warnings:[{code:'recent_history_unavailable'}],_ui_engine_revision:7,status:{stages:[]}};
const context=vm.createContext({console,Promise,WeakMap,window:{__TAURI__:{dialog:{open:async()=>null}}},document:{addEventListener(){},body:{classList:{remove(){}}}},$:node,el:(tag,attrs,...children)=>({tag,...attrs,children}),toast:m=>toasts.push(m),state:{project:null},agent:{},acFeedback:{actions:new WeakMap(),acknowledge:async(revision,project)=>calls.push({ack:revision,project})},invoke:async(command,params)=>{calls.push({command,params});return result;},openProject:async()=>{throw Error('New must apply CLI result without issuing another Open');},applyOpenedProject:async opened=>{calls.push({applied:opened});},animationReset(){},renderTarget(){},renderBlocks(){},renderStoryboard(){},setView:async()=>{},agentToggle(){}});
vm.runInContext(readFileSync(new URL('../ui/home.js',import.meta.url),'utf8'),context);
node('new-name').value='A&B';node('new-root').value='/chosen';node('new-audio').value='/chosen/song.wav';
await node('new-form').handlers.submit({preventDefault(){},submitter:node('new-create')});
assert.equal(calls.filter(c=>c.command==='project_create').length,1);
assert.equal(calls.find(c=>c.applied)?.applied,result,'New preserves the entire intake result for rendering');
assert.ok(toasts.some(m=>m.includes('No valid transcript yet')),'New surfaces intake problems');
assert.ok(!toasts.includes('Project created'),'incomplete intake is not reported as finished');
assert.equal(node('new-create').disabled,false);
vm.runInContext('renderWhere()',context);
assert.ok(!node('new-where').textContent.includes('/a-b/'),'destination is not invented with a second slug implementation');
// Run the real apply/open functions from app.js, leaving unrelated view setup out of the fixture.
const source=readFileSync(new URL('../ui/app.js',import.meta.url),'utf8');
const start=source.indexOf('async function openProject('),end=source.indexOf('document.querySelectorAll(".tab")',start);
vm.runInContext(source.slice(start,end),context);
calls.length=0;
await vm.runInContext('openProject("/chosen/ab/project/A&B.magic")',context);
assert.equal(calls.filter(c=>c.command==='project_open').length,1);
assert.deepEqual(calls.find(c=>c.ack),{ack:7,project:result.project});
assert.equal(context.state.data.problems[0],'No valid transcript yet');
assert.equal(context.state.project,result.project);
console.log('PASS: New applies full CLI result once, shows intake problems, uses returned destination, Open renders and acknowledges once');
// A project changed by MCP/CLI refreshes from engine state, never by issuing New/Open again.
vm.runInContext(readFileSync(new URL('../ui/bridge.js',import.meta.url),'utf8'),context);
context.state.project=null;
calls.length=0;
context.invoke=async(command,params)=>{calls.push({command,params});return {status:{stages:[]}};};
await vm.runInContext('window.magicBridge.refresh("project.intake","/chosen/ab/project/A&B.magic",7,{problems:["Needs captions"],intake_ready:false})',context);
assert.equal(calls.filter(c=>c.command==='project_open'||c.command==='project_create').length,0,'external transition refresh does not recursively launch the CLI');
assert.equal(calls.filter(c=>c.command==='project_state').length,1);
assert.equal(context.state.project,result.project);
assert.equal(context.state.data.problems[0],'Needs captions');
assert.equal(context.state.projectReceipt.problems[0],'Needs captions','receipt remains available when later state reads refresh the view');
console.log('PASS: external CLI/MCP project refresh renders and acknowledges without recursive Open');
