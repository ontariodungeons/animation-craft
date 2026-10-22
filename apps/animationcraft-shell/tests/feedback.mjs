// Exercise the actual feedback/invoke code; native reports are captured, never fabricated into a project.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const reports=[],native=[],frames=[],intervals=[],listeners={};
const pane={textContent:'Rendered project',getBoundingClientRect:()=>({width:900,height:600})};
const context=vm.createContext({console,state:{project:'/fixture/test.acproj',view:'project',page:0},
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
function click(navigation=false){const node={id:navigation?'sb-next':'sb-save-version',dataset:{},getAttribute:()=>null,matches:()=>navigation,textContent:''};const event={target:{closest:()=>node}};listeners.click(event);return event;}
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
