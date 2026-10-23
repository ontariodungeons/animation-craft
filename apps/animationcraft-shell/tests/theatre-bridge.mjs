import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {theatreProjectId} from '../frontend/project-id.mjs';
const firstId=await theatreProjectId('/long/project/path/Bumble Bee.acproj');
assert.ok(firstId.length<=32);
assert.equal(firstId,await theatreProjectId('/long/project/path/Bumble Bee.acproj'));
assert.notEqual(firstId,await theatreProjectId('/different/project/Bumble Bee.acproj'));
const messages=[],listeners={},nodes=new Map();
const frameWindow={postMessage:message=>messages.push(message)};
const context=vm.createContext({window:{addEventListener:(type,handler)=>listeners[type]=handler},location:{origin:'tauri://localhost'},
 el:(tag,props)=>{const node={...props,contentWindow:tag==='iframe'?frameWindow:undefined};if(props.id)nodes.set(props.id,node);return node;},
 $:id=>nodes.get(id),state:{project:'/project/Bee.acproj'},motion:{character:'bee',selected:'antenna',meta:{layers:[{id:'antenna'}]},sample:{duration:5},time:0},
 animationDraw(){},animationPause(){}});
vm.runInContext(readFileSync(new URL('../ui/theatre.js',import.meta.url),'utf8'),context);
vm.runInContext('theatreMount({append(){}})',context);
listeners.message({source:{},data:{magicTheatre:true,type:'loaded'}});
assert.equal(messages.length,0,'unrelated iframe cannot control animation');
listeners.message({source:frameWindow,data:{magicTheatre:true,type:'loaded'}});
assert.equal(messages[0].part,'antenna');
listeners.message({source:frameWindow,data:{magicTheatre:true,type:'seek',time:12}});
assert.equal(context.motion.time,5,'Theatre seek clamps to actual native track duration');
listeners.message({source:frameWindow,data:{magicTheatre:true,type:'values',part:'other',rotation:30}});
assert.equal(vm.runInContext('magicTheatre.rotation',context),0,'stale selected-part values cannot edit another part');
messages.length=0;
context.animationDraw=()=>vm.runInContext('theatreSeek(motion.time)',context);
listeners.message({source:frameWindow,data:{magicTheatre:true,type:'values',part:'antenna',rotation:30}});
assert.equal(messages.filter(m=>m.type==='seek').length,0,'Theatre value redraw must not send an old native time back and pause Theatre playback');
const operations=[];context.ctx={translate:(...v)=>operations.push(['translate',...v]),rotate:v=>operations.push(['rotate',v])};
vm.runInContext('theatreTransform(ctx,{id:"antenna",pivot:[12,24]})',context);
assert.deepEqual(operations,[['translate',12,24],['rotate',Math.PI/6],['translate',-12,-24]]);
console.log('PASS: isolated Theatre source, captured part, native duration and original pivot');
