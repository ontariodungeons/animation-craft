import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const nodes=new Map(),calls=[];
const el=(tag,attrs={},...children)=>{const n={tag,...attrs,children:children.flat(),style:{setProperty(){}},events:{},append(...c){this.children.push(...c)},addEventListener(k,f){this.events[k]=f},setAttribute(k,v){this[k]=v},get options(){return this.children.filter(c=>c?.tag==='option')},get value(){return this._value??attrs.value??this.options[0]?.value??''},set value(v){this._value=String(v)}};if(attrs.id)nodes.set(attrs.id,n);return n;};
const c=vm.createContext({uiConfig:JSON.parse(readFileSync(new URL('../ui/components.json',import.meta.url),'utf8')),console,el,$:id=>nodes.get(id),ICONS:{chev:''},window:{addEventListener(){}},localStorage:{getItem:()=>null,setItem(){}},state:{project:'/generic/Study.magic'},toast:m=>calls.push(m)});
vm.runInContext(readFileSync(new URL('../ui/animation.js',import.meta.url),'utf8'),c);
vm.runInContext(`animationPause=()=>{motion.playing=false};animationDraw=()=>{};motion.sample={duration:3,fps:24};motion.time=1;animationStep(-1);`,c);assert.equal(vm.runInContext('motion.time',c),1-1/24);
vm.runInContext('animationSeek(999)',c);assert.equal(vm.runInContext('motion.time',c),3);vm.runInContext('animationSeek(-1)',c);assert.equal(vm.runInContext('motion.time',c),0);
vm.runInContext(`motion.meta={layers:[{id:'root-a',label:'Paper base',parent:null},{id:'root-b',label:'Second base',parent:null},{id:'detail',parent:'root-a'}]};motion.points=[[0,0],[12,14]];pathControls();`,c);
assert.equal(nodes.get('motion-path-layer').options.length,2,'path target exposes actual roots only');
nodes.get('motion-path-layer').value='root-b';nodes.get('motion-path-layer').onchange();assert.equal(vm.runInContext('motion.pathLayer',c),'root-b');
nodes.get('motion-path-point').value='1';nodes.get('motion-path-point').onchange();nodes.get('motion-path-x').value='25';nodes.get('motion-path-x').onchange();assert.equal(vm.runInContext('motion.points[1][0]',c),25,'point editor changes actual draft route');
console.log('PASS transport/path: frame stepping and bounded seeking; actual roots and draft point editing');

// Action export must capture the visible sampler settings and refuse incomplete results.
const exported=[];Object.assign(c,{theatreRequireSaved(){},fileButton:chip=>({file:chip.path}),invoke:async(_,r)=>{exported.push(r);return {ok:true,outputs:[{file:'/generic/wave.webm'}]}}});
vm.runInContext(`motion.character='puppet';motion.track='wave';motion.rest=true;motion.amount=35;motion.speed=1.5;motion.sample={duration:3,fps:24};`,c);
assert.equal(typeof c.motionExportControl,'function','export control must use captured sampler settings');let control=c.motionExportControl();await control.button.onclick();assert.equal(JSON.stringify(exported[0].params),JSON.stringify({character:'puppet',track:'wave',rest:true,amount:35,duration:2}));assert.match(control.status.textContent,/Track ready/);
c.theatreRequireSaved=()=>{throw Error('Save native motion first')};control=c.motionExportControl();await control.button.onclick();assert.equal(exported.length,1);assert.match(control.status.textContent,/Save native/);
c.theatreRequireSaved=()=>{};c.invoke=async()=>({ok:true,outputs:[]});control=c.motionExportControl();await control.button.onclick();assert.doesNotMatch(control.status.textContent,/Track ready/);assert.match(control.status.textContent,/output|confirmation/i);console.log('PASS action export: captured rest/amount/rate, native-save guard, no empty-output success');

let complete;const pendingCalls=[];c.invoke=async(_,request)=>{pendingCalls.push(request);return new Promise(resolve=>complete=resolve)};control=c.motionExportControl();const pending=control.button.onclick(),duplicate=control.button.onclick();assert.equal(pendingCalls.length,1,'export submission is single-flight');c.state.project='/generic/Other.magic';vm.runInContext("motion.character='other'",c);complete({ok:true,outputs:[{file:'/generic/captured.webm'}]});await Promise.all([pending,duplicate]);assert.equal(vm.runInContext('motion.lastExport.project',c),'/generic/Study.magic');assert.equal(vm.runInContext('motion.lastExport.params.character',c),'puppet');assert.doesNotMatch(control.status.textContent,/Track ready/,'stale completion cannot announce success in another project');console.log('PASS export receipt: single-flight and captured project/character survive navigation');
