import vm from 'node:vm';import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';
const storage=new Map(),c=vm.createContext({console,JSON,localStorage:{getItem:k=>storage.get(k)??null,setItem:(k,v)=>storage.set(k,v)},window:{addEventListener(){}},state:{project:'/generic/A.magic'}});
vm.runInContext(readFileSync(new URL('../ui/animation.js',import.meta.url),'utf8'),c);
vm.runInContext(`motion.project=state.project;motion.character='puppet';motion.track='wave';motion.selected='paper';motion.time=1.25;motion.speed=1.5;motion.amount=65;motion.rest=false;motion.points=[[1,2],[3,4]];animationRemember();motion.character=null;motion.track='';const resume=animationRestore();`,c);
assert.equal(vm.runInContext(`resume.character==='puppet'&&resume.track==='wave'&&resume.selected==='paper'&&resume.time===1.25&&resume.speed===1.5&&resume.amount===65&&resume.points.length===2`,c),true);
c.state.project='/generic/B.magic';assert.equal(vm.runInContext('animationRestore()',c),null,'selection must not leak across projects');
for(let i=0;i<30;i++){c.state.project=`/generic/${i}.magic`;vm.runInContext('motion.project=state.project;motion.character="puppet";animationRemember()',c)}assert.ok(JSON.parse(storage.get('animation.sessions')).length<=20,'stored UI history bounded');
console.log('PASS session: project-scoped selection, preview settings and route restore with bounded history');
// An explicit Project asset choice must outrank remembered selection.
c.theatreCanLeave=()=>true;c.theatreRequireSaved=()=>{};c.invoke=async()=>({assets:[{id:'puppet'},{id:'chosen-object'}]});
vm.runInContext(`animationShell=()=>{};animationLoad=async()=>{};setView=async()=>animationOpen();`,c);
await vm.runInContext(`openAnimation('chosen-object')`,c);assert.equal(vm.runInContext('motion.character',c),'chosen-object','explicit asset choice outranks session restore');console.log('PASS explicit object choice outranks restored selection');
