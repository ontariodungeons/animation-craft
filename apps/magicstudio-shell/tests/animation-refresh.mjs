// Real refresh/selection handlers with generic assets and mismatched old/new layer sets.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=readFileSync(new URL('../ui/animation.js',import.meta.url),'utf8');
function fixture(assets=[{id:'puppet',name:'Paper puppet'}]){
 const calls=[],nodes=new Map(['motion-caption','motion-scrub','motion-inspector','motion-stage'].map(id=>[id,{replaceChildren(){},style:{}}]));
 const c=vm.createContext({console,JSON,Map,Set,Promise,localStorage:{getItem:()=>null},window:{addEventListener(){}},state:{project:'/generic/Study.magic',view:'animation'},$:id=>nodes.get(id),Image:class{set src(v){this.onload()}},convertFileSrc:p=>p,toast:m=>calls.push(['toast',m]),refreshPartDrafts:async()=>{},invoke:async(_,r)=>{calls.push([r.id,r.params]);if(r.id==='project.list_assets')return {assets};if(r.id==='motion.tracks'){assert.equal(r.params.character,'puppet','refresh must reconcile invalid character from registry first');return {character:'puppet',layers:[{id:'base'},{id:'top'}],tracks:[{id:'wave',label:'Wave',duration:3},{id:'bow',label:'Bow',duration:2}]}}if(r.id==='motion.sample')return {character:'puppet',track:r.params.track,duration:3,fps:30,frames:[]};throw Error('Unexpected '+r.id)}});
 vm.runInContext(source,c);Object.assign(c,{el:()=>({}),animationDraw(){},animationPause(){},theatreRequireSaved(){},refreshPartDrafts:async()=>{},timelineRender:()=>vm.runInContext(`if(motion.sample&&motion.meta.layers.length!==motion.sample.layerCount)throw Error('old sample / new layers');`,c)});
 vm.runInContext(`animationControls=()=>{if(motion.sample&&motion.meta.layers.length!==motion.sample.layerCount)throw Error('old sample / new layers');};motion.character='puppet';motion.track='bow';motion.rest=false;motion.selected='top';motion.time=1;motion.meta={layers:[{id:'old'}]};motion.sample={layerCount:1};`,c);
 // Fresh sampler's fixture carries its own coherent layer count.
 const invoke=c.invoke;c.invoke=async(k,r)=>{const v=await invoke(k,r);if(r.id==='motion.sample')v.layerCount=2;return v};
 return {c,calls};
}
let f=fixture();await vm.runInContext('animationRefresh()',f.c);assert.equal(vm.runInContext(`motion.track==='bow'&&motion.selected==='top'&&motion.time===1&&!motion.rest`,f.c),true,'refresh preserves valid authoring selection/time');
f=fixture();vm.runInContext(`motion.character='deleted';motion.track='missing';motion.selected='deleted-part';`,f.c);await vm.runInContext('animationRefresh()',f.c);assert.equal(vm.runInContext(`motion.character==='puppet'&&motion.track==='wave'&&motion.selected===null`,f.c),true);
f=fixture([]);await vm.runInContext('animationRefresh()',f.c);assert.equal(vm.runInContext('motion.meta===null&&motion.sample===null&&motion.character===null',f.c),true);assert.equal(f.calls.some(([id])=>id==='motion.tracks'),false,'empty project must not query stale character');
console.log('PASS refresh: coherent metadata/sample; valid selection preserved; stale/empty assets reconciled');

f=fixture();vm.runInContext(`motion.sample=null;motion.meta={layers:[{id:'base'},{id:'top'}],tracks:[{id:'bow'}]};`,f.c);f.c.invoke=async()=>({layer_ids:['top','base'],duration:3,fps:30,frames:[]});await vm.runInContext('animationSample()',f.c);assert.equal(vm.runInContext('motion.sample',f.c),null,'mismatched sampler layer IDs must not publish transforms for the wrong images');assert.ok(f.calls.some(([id,message])=>id==='toast'&&/layer order/i.test(message)));console.log('PASS sampling: changed layer order cannot mix transforms with stale images');
