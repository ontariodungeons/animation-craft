// Exercise the actual feedback/invoke code; native reports are captured, never fabricated into a project.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const reports=[],native=[],frames=[],intervals=[],listeners={};
const pane={textContent:'Rendered project',getBoundingClientRect:()=>({width:900,height:600})};
const context=vm.createContext({console,updateAppFooter(){},state:{project:'/fixture/test.magic',view:'project',page:0},
 document:{body:{classList:{contains:()=>false}},querySelectorAll:()=>[],querySelector:()=>pane,getElementById:()=>null,addEventListener:(event,fn)=>listeners[event]=fn},
 addEventListener:()=>{},getComputedStyle:()=>({display:'block'}),queueMicrotask,
 requestAnimationFrame:fn=>frames.push(fn),clearTimeout:()=>{},setTimeout:()=>1,setInterval:(fn,ms)=>intervals.push({fn,ms}),
 __TAURI__:{core:{invoke:async(id,args)=>{reports.push({id,...args});return {};}}},
 nativeInvoke:async(command,params)=>{native.push({command,params});return {_ui_engine_revision:2};}});
context.window=context;
vm.runInContext(readFileSync(new URL('../ui/feedback.js',import.meta.url),'utf8'),context);
const app=readFileSync(new URL('../ui/app.js',import.meta.url),'utf8');
vm.runInContext(app.slice(app.indexOf('async function invoke('),app.indexOf('const $ =')),context);
assert.equal(intervals[0]?.ms,1000,'Frontend must refresh its own observation, independently of Booth polling');
context.acFeedback.ack=1;
const before=reports.length;await intervals[0].fn();
assert.equal(reports.length,before+1);assert.equal(reports.at(-1).report.event,undefined,'Heartbeat must not invent activity');assert.equal(context.acFeedback.ack,1,'Heartbeat must not acknowledge unseen engine work');
function click(navigation=false){const node={id:navigation==='tab'?'Studio':navigation?'sb-next':'sb-save-version',dataset:navigation==='tab'?{view:'studio'}:{},getAttribute:()=>null,matches:selector=>navigation==='tab'?selector.includes('.tab[data-view]'):selector.includes('#sb-next')&&!!navigation,textContent:''};const event={target:{closest:()=>node}};listeners.click(event);return event;}
const save=click();
assert.equal(context.acFeedback.action?.request_id,save.acAction.request_id,'Static handlers must receive the captured click request');
await vm.runInContext("invoke('engine_command',{id:'storyboard.save_version',params:{}})",context);
assert.equal(native[0].params.params._request_id,save.acAction.request_id);
assert.equal(context.acFeedback.action,undefined,'Click context must not leak into unrelated callbacks');
const nav=click(true);context.state.page=1;frames.shift()();await Promise.resolve();
const receipt=reports.find(r=>r.report.event?.stage==='ui_navigation_applied')?.report.event;
assert.equal(receipt?.request_id,nav.acAction.request_id);assert.equal(receipt?.page,2);
assert.equal(context.acFeedback.ack,1,'Navigation must not fabricate an engine revision');
const count=reports.filter(r=>r.report.event?.stage==='ui_navigation_applied').length;
click(true);context.acFeedback.errors.push({message:'Handler failed'});frames.shift()();await Promise.resolve();
assert.equal(reports.filter(r=>r.report.event?.stage==='ui_navigation_applied').length,count,'Failed navigation must not report completion');
console.log('PASS: real heartbeat without activity/ack fabrication, static click request propagation, correlated navigation after render and no false completion on error');

context.acFeedback.errors=[];
const tab=click('tab');
assert.equal(reports.at(-1).report.event.interaction,'navigation','Existing third tab must be classified as navigation, not unknown control');

context.document.querySelector=selector=>selector==='#view-studio'?null:pane;
context.placeInk=()=>{};context.serverUrl=()=>'http://localhost:3058/#project/hyperframes';
// This fixture extracts only navigation handlers; the workstation catalog is rendered separately.
context.renderMagicAssetCatalog=()=>{};
context.$=()=>({classList:{toggle:()=>{},add:()=>{}}});
let completeNative;
context.nativeInvoke=async command=>command==='studio_show'?new Promise(resolve=>{completeNative=resolve;}):{};
vm.runInContext(app.slice(app.indexOf('async function showStudio()'),app.indexOf('// ---- Studio server')),context);
context.tabAction=tab.acAction;
const beforeTab=reports.filter(r=>r.report.event?.stage==='ui_navigation_applied').length;
const showing=vm.runInContext("setView('studio',tabAction)",context);
assert.equal(frames.length,1,'View tabs must use their asynchronous handler, not the generic first-frame completion');
frames.shift()();await new Promise(setImmediate);
assert.equal(reports.filter(r=>r.report.event?.stage==='ui_navigation_applied').length,beforeTab,'Do not acknowledge before the existing embedded-view operation finishes');
completeNative({});await showing;
const completed=reports.at(-1).report.event;
assert.equal(completed.stage,'ui_navigation_applied');assert.equal(completed.request_id,tab.acAction.request_id);assert.equal(completed.view,'studio');assert.equal(context.acFeedback.ack,1);
context.nativeInvoke=async()=>{throw new Error('Native view failed');};context.toast=()=>{};
const failed=vm.runInContext("setView('studio',tabAction)",context);frames.shift()();await assert.rejects(failed,/Native view failed/);
assert.equal(reports.filter(r=>r.report.event?.stage==='ui_navigation_applied').length,beforeTab+1,'A failed native view must not create navigation completion');
console.log('PASS: Studio tab emits one correlated navigation receipt after native view completion; failure and engine revisions remain truthful');

const field=(id,value,defaultValue)=>({id,value,defaultValue,tagName:'INPUT',getBoundingClientRect:()=>({width:90,height:20}),closest:()=>null});
context.document.querySelectorAll=()=>[field('motion-scrub','2.236','0'),field('agent-input','unsent draft','')];
assert.deepEqual(Array.from(vm.runInContext('feedbackSnapshot().draft_fields',context)),['agent-input'],'playhead navigation is not an unsaved document edit; text drafts remain protected');

context.document.querySelectorAll=()=>[];context.state.project='/fixture/test.magic';context.acFeedback.errors=[];
let timeoutFrame;context.setTimeout=fn=>{timeoutFrame=fn;return 1;};
const priorAck=context.acFeedback.ack;const deferred=context.acFeedback.acknowledge(99,context.state.project);timeoutFrame();await deferred;
assert.equal(context.acFeedback.ack,priorAck,'background frame timeout cannot acknowledge an unseen revision');
assert.equal(context.acFeedback.phase,'ready','handler readiness must not hang indefinitely on an inactive workspace');
assert.equal(reports.at(-1).report.event.stage,'render_deferred');
console.log('PASS: deferred background frame preserves unknown render receipt without blocking handler readiness');
context.theatreFeedback=()=>({live_edits:'needs_save'});
assert.ok(vm.runInContext('feedbackSnapshot()',context).draft_fields.includes('theatre-motion'),'iframe-local interactions must reach the existing native dirty guard');
context.theatreFeedback=()=>({live_edits:'saved_snapshot'});
assert.ok(!vm.runInContext('feedbackSnapshot()',context).draft_fields.includes('theatre-motion'),'a matching saved snapshot clears the conservative draft marker');
console.log('PASS: Theatre interactions participate in native close/project dirty fields');
