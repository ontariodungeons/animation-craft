// Original shared frames; no inference, polling or navigation belongs to these indicators.
function createCompanions(frames) {
 const media=matchMedia('(prefers-reduced-motion: reduce)'),seen=new Set(),jobs=new Map();
 let enabled=localStorage.getItem('magic.companions')==='true',project=null,timer=null,step=0,sequence=0;
 const nodes={},pets={},anchors={};
 const home=name=>({destination:name==='gaffer'?'artwork':'chat',status:'home',command:null,request:null,reason:'Home',key:null});
 for(const name of ['sproket','gaffer']){
  const node=document.createElement('pre');node.id=`companion-${name}`;node.className='ambient-companion';
  node.style.pointerEvents='none';node.setAttribute('aria-hidden','true');node.setAttribute('inert','');nodes[name]=node;pets[name]=home(name);
 }
 const bounded=(map,key,value)=>{map instanceof Map?map.set(key,value):map.add(key);while(map.size>64)map.delete(map.keys().next().value);};
 const tab=view=>[...document.querySelectorAll('.tab[data-view]')].find(n=>n.dataset.view===view);
 const anchor=destination=>destination==='chat'?document.querySelector('.agent-title'):destination==='artwork'?document.querySelector('#magic-image-toggle'):tab(destination);
 const visible=name=>{const a=anchors[name];return enabled&&!!project&&!document.hidden&&!!a&&a.getBoundingClientRect().width>0&&!a.closest?.('[hidden],[aria-hidden=true]');};
 function frame(index){for(const name of Object.keys(nodes)){const text=frames.frames[name][index%frames.frames[name].length];if(visible(name)&&nodes[name].textContent!==text)nodes[name].textContent=text;}}
 function stop(){if(timer!==null)clearTimeout(timer);timer=null;step=0;frame(0);}
 function render(){
  let moved=false;
  for(const name of Object.keys(pets)){
   const a=anchor(pets[name].destination);if(a&&anchors[name]!==a){a.after(nodes[name]);anchors[name]=a;moved=true;}
   const hidden=!visible(name);if(nodes[name].hidden!==hidden)nodes[name].hidden=hidden;
  }
  if(moved&&typeof placeInk==='function')placeInk();
 }
 function sync(){
  if(project!==state.project){stop();project=state.project;seen.clear();jobs.clear();for(const name of Object.keys(pets))pets[name]=home(name);}
  render();if(!enabled||document.hidden||media.matches||!Object.keys(nodes).some(visible))stop();
  if(timer===null)frame(0);
 }
 function burst(){
  if(timer!==null)clearTimeout(timer);timer=null;step=0;render();
  if(!enabled||document.hidden||media.matches||!Object.keys(nodes).some(visible)){stop();return;}
  const tick=()=>{step++;if(step>=3){timer=null;step=0;frame(0);return;}frame(step);timer=setTimeout(tick,500);};
  timer=setTimeout(tick,500);
 }
 const noise=id=>!id||/^(agent\.|ui\.|center\.|project\.(open|close|root|recent)|motion\.(sample|tracks)|blender\.(actions|prepare))/.test(id)||/(^|[.])(status|list|read|state|log|session|models|feedback|preview|inspect|probe|capabilities|get)([.]|$)|\.(get_|list_|read_)/.test(id);
 const keyOf=e=>JSON.stringify([e.command,...['character','asset','part','track','scene','placement','job','path'].map(k=>e.params?.[k]??null)]);
 function event(input){
  // A late completion may never switch the active project or erase its current explanation.
  if(input.project!==state.project||input.result?.project&&input.result.project!==state.project)return;
  if(!enabled||!state.project)return;
  if(project!==state.project)sync();
  let e=input;
  if(/\.status$/.test(e.command)&&e.result?.job){
   let previous=jobs.get(e.result.job);
   if(!previous){if(['running','cancelling'].includes(e.result.state))bounded(jobs,e.result.job,{event:{...e,command:e.command.replace(/status$/,'start'),request:e.result.job},state:e.result.state});return;}
   if(previous.state===e.result.state)return;
   previous.state=e.result.state;if(!['completed','failed','cancelled'].includes(e.result.state))return;
   e={...previous.event,request:`${previous.event.request}:${e.result.job}:${e.result.state}`,phase:e.result.state==='failed'?'error':'success',result:e.result,error:e.result.error||e.result.detail};
  }else if(noise(e.command)){
   // Chat can itself fail, but asking why must not resolve or relocate an existing pin.
   if(e.command!=='agent.send')return;
   if(e.phase==='error'){if(['failed','blocked'].includes(pets.sproket.status))return;}
   else if(e.phase!=='success'||pets.sproket.command!=='agent.send')return;
  }
  if(!tab(e.view))return;
  const signature=JSON.stringify([e.project,e.command==='image.generate'&&e.phase==='success'?e.result?.asset_id||e.request:e.request,e.command,e.phase,e.result?.state]);if(seen.has(signature))return;bounded(seen,signature);sync();
  if(e.result?.job&&e.command.endsWith('.start'))bounded(jobs,e.result.job,{event:e,state:e.result.state});
  const failed=e.phase==='error'||e.result?.ok===false||['failed','blocked'].includes(e.result?.state);
  const pending=['submitted','running','cancelling'].includes(e.result?.state);
  const key=keyOf(e),status=failed?(e.result?.state==='blocked'?'blocked':'failed'):e.phase==='start'||pending?'working':'completed';
  if(e.command!=='agent.send')pets.gaffer={destination:e.command==='image.generate'&&!failed&&e.phase!=='start'?'artwork':e.view,status,command:e.command,request:e.request,reason:`${status}: ${e.command}`,key};
  if(failed)pets.sproket={destination:e.view,status,command:e.command,request:e.request,reason:String(e.error||e.result?.error||`${status}: ${e.command}`).slice(0,180),key};
  else if(e.phase!=='start'&&!pending&&pets.sproket.key===key)pets.sproket={...home('sproket'),status:'resolved',command:e.command,request:e.request,reason:`Resolved: ${e.command}`};
  burst();
 }
 document.addEventListener('visibilitychange',sync);media.addEventListener('change',sync);
 sync();
 return {nodes,event,sync,capture(command,params={}){const e={command,params,project:state.project,view:state.view,request:params._request_id||`ambient-${Date.now()}-${++sequence}`};event({...e,phase:'start'});return e;},setEnabled(value){enabled=!!value;localStorage.setItem('magic.companions',String(enabled));sync();},snapshot(){return JSON.parse(JSON.stringify({enabled,project,reducedMotion:media.matches,hidden:document.hidden,animating:timer!==null,gaffer:pets.gaffer,sproket:pets.sproket,dedupe:seen.size,jobs:jobs.size}));}};
}
async function initCompanions(){
 const response=await fetch('companions.frames.json');if(!response.ok)throw new Error('Companion frames unavailable');
 const controller=createCompanions(await response.json());window.magicCompanions=controller;
 const setting=document.getElementById('ambient-companions');setting.checked=controller.snapshot().enabled;
 setting.addEventListener('change',()=>controller.setEnabled(setting.checked));
}

if(document.getElementById('ambient-companions'))initCompanions().catch(error=>console.warn(String(error)));
