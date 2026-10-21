// The engine samples motion; this view only draws images using returned matrices.
const motion = { project:null, character:null, meta:null, sample:null, images:[], time:0, playing:false, zoom:1, fit:true, track:'', rest:true, amount:100, speed:1, selected:null, loading:false, request:0 };
const motionCommand = (id, params={}) => invoke('engine_command',{id,params});
const MOTION_ICONS = {
 play:'<path d="m8 5 11 7-11 7z"/>', pause:'<path d="M8 5v14M16 5v14"/>',
 reset:'<path d="M3 10a9 9 0 1 1 2 9M3 4v6h6"/>', fit:'<path d="M8 3H3v5M16 3h5v5M3 16v5h5M21 16v5h-5"/>',
 minus:'<path d="M5 12h14"/>', plus:'<path d="M5 12h14M12 5v14"/>', eye:'<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/>',
 layers:'<path d="m12 3 9 5-9 5-9-5zM3 13l9 5 9-5M3 18l9 5 9-5"/>', export:'<path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>', step:'<path d="m5 5 10 7-10 7zM19 5v14"/>'
};
function motionButton(id,label,icon,fn){return el('button',{id,class:'icon',title:label,'aria-label':label,html:`<svg viewBox="0 0 24 24">${MOTION_ICONS[icon]}</svg>`,onclick:fn});}
function motionSection(title,body,open=false){const node=el('details',{class:'motion-section'},el('summary',{},title),el('div',{class:'motion-section-body'},body));node.open=open;return node;}
function motionRange(id,label,min,max,step,value,unit,change){const out=el('output',{},`${value}${unit}`);const input=el('input',{id,type:'range',min,max,step,value,'aria-label':label,title:`${label}: ${min}–${max}${unit}`,oninput:e=>{out.textContent=`${e.target.value}${unit}`;change(Number(e.target.value));}});return el('label',{class:'motion-range'},el('span',{},label,out),input);}
async function openAnimation(character){motion.character=character;motion.project=null;await setView('animation');}
async function animationOpen(){
 if(motion.project===state.project && motion.meta){animationDraw();return;}
 motion.project=state.project;motion.meta=null;motion.sample=null;motion.time=0;motion.rest=true;
 animationShell();
 try{
  const assets=await motionCommand('project.list_assets');
  const list=assets.assets||assets.project_assets||[];
  motion.assets=list;
  if(!list.some(a=>a.id===motion.character))motion.character=list[0]?.id;
  if(!motion.character){animationEmpty('Bring in your artwork','Choose an object from the Project view.');return;}
  await animationLoad();
 }catch(e){animationEmpty('Artwork needs attention',String(e));}
}
function animationEmpty(title,detail){$('motion-stage').replaceChildren(el('div',{class:'motion-empty'},el('strong',{},title),el('p',{class:'muted'},detail),el('button',{class:'start',onclick:()=>setView('project')},'Open Project')));}
function animationShell(){
 const view=$('view-animation');view.replaceChildren();
 const inspector=el('aside',{class:'motion-inspector',id:'motion-inspector','aria-label':'Animation controls'});
 const stage=el('div',{class:'motion-stage',id:'motion-stage'},el('canvas',{id:'motion-canvas','aria-label':'Character animation canvas',tabindex:0}));
 const toolbar=el('div',{class:'motion-stagebar'},el('span',{id:'motion-caption',class:'muted'},'Loading artwork…'),el('div',{class:'motion-tools'},
  motionButton('motion-fit','Fit artwork','fit',()=>{motion.fit=true;motion.zoom=1;animationDraw();}),
  el('button',{class:'icon',id:'motion-actual',title:'Actual size · 100%','aria-label':'Actual size',onclick:()=>{motion.fit=false;motion.zoom=1;animationDraw();}},'1:1'),
  motionButton('motion-minus','Zoom out','minus',()=>{motion.zoom/=1.2;animationDraw();}),motionButton('motion-plus','Zoom in','plus',()=>{motion.zoom*=1.2;animationDraw();})));
 const transport=el('div',{class:'motion-transport'},motionButton('motion-play','Play motion','play',animationPlay),motionButton('motion-rewind','Return to start','reset',()=>{animationPause();motion.time=0;animationDraw();}),motionButton('motion-step','Next frame','step',()=>{animationPause();motion.time=Math.min(motion.sample?.duration||0,motion.time+1/(motion.sample?.fps||30));animationDraw();}),el('span',{id:'motion-time',class:'motion-time'},'0:00.000'),el('input',{id:'motion-scrub',type:'range',min:0,max:1,step:0.001,value:0,'aria-label':'Scrub motion',oninput:e=>{animationPause();motion.time=Number(e.target.value);animationDraw();}}));
 view.append(inspector,el('div',{class:'motion-main'},toolbar,stage,transport));
 new ResizeObserver(()=>animationDraw()).observe(stage);
 $('motion-canvas').addEventListener('click',()=>{$('motion-layers')?.setAttribute('open','');});
 $('motion-canvas').addEventListener('keydown',e=>{if(e.key===' '){e.preventDefault();animationPlay();}});
}
async function animationLoad(){
 animationPause();const request=++motion.request;motion.loading=true;
 try{
  const meta=await motionCommand('motion.tracks',{character:motion.character});if(request!==motion.request)return;
  motion.meta=meta;motion.track=meta.tracks?.[0]?.id||'';motion.time=0;
  motion.images=await Promise.all((meta.layers||[]).map(l=>new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(new Error(`Could not load ${l.label}`));img.src=convertFileSrc(l.file);})));if(request!==motion.request)return;
  animationControls();await animationSample();
 }catch(e){animationEmpty('Artwork needs preparation',String(e));}finally{motion.loading=false;}
}
function animationControls(){
 const box=$('motion-inspector');box.replaceChildren();const meta=motion.meta;
 const picker=el('select',{id:'motion-object','aria-label':'Animated object',title:'Choose an object',onchange:async e=>{motion.character=e.target.value;await animationLoad();}},(motion.assets||[]).map(a=>el('option',{value:a.id},a.name||a.id)));picker.value=motion.character;
 box.append(el('div',{class:'motion-object-head'},el('span',{class:'dot ok',title:'Artwork loaded'}),picker));
 const tracks=el('select',{id:'motion-track','aria-label':'Motion track',title:'Choose a motion track',onchange:async e=>{motion.track=e.target.value;motion.rest=!e.target.value;motion.time=0;await animationSample();}},el('option',{value:''},'Rest pose'),(meta.tracks||[]).map(t=>el('option',{value:t.id},t.id.replace(/^.*?-beat\d+-/, '').replaceAll('-',' '))));tracks.value=motion.rest?'':motion.track;
 box.append(motionSection('Action',tracks,true));
 box.append(motionSection('Motion & timing',[
 motionRange('motion-speed','Playback speed',0.25,2.25,0.05,motion.speed,'×',v=>motion.speed=v),
 motionRange('motion-amount','Motion amount',0,100,1,motion.amount,'%',v=>{motion.amount=v;clearTimeout(motion.amountTimer);motion.amountTimer=setTimeout(animationSample,140);})],true));
 const layers=el('div',{class:'motion-layers'});
 for(const l of [...meta.layers].reverse()){
  const row=el('div',{class:'motion-layer',title:l.id});const shown=l.state?.visible!==false;
  row.append(el('img',{src:convertFileSrc(l.file),alt:''}),el('span',{class:'motion-layer-name'},l.label),motionButton(`motion-visible-${l.id}`,`${shown?'Hide':'Show'} ${l.label}`,'eye',async()=>{await motionCommand('part.set_state',{part:l.id,visible:!shown});await animationRefresh();}));
  row.lastChild.setAttribute('aria-pressed',String(shown));if(!shown)row.classList.add('hidden-layer');layers.append(row);
 }
 const section=motionSection('Layers',layers,true);section.id='motion-layers';box.append(section);
 const controls=meta.controls?.joints||[];
 box.append(motionSection('Expression & pose',el('p',{class:'muted'},controls.length?'This rig exposes joint bindings.':'This rig uses authored motion tracks. Expression controls require explicit rig bindings.')));
 box.append(motionSection('Quality checks',el('p',{class:'muted'},`${meta.layers.length} layers · ${meta.tracks.length} motion tracks`)));
 box.append(motionSection('Export & integrations',el('button',{class:'ghost',id:'motion-export',title:'Open the layered motion specification','aria-label':'Open motion specification',onclick:()=>openPath(meta.spec)},'Motion specification')));
}
async function animationSample(){
 if(!motion.meta?.tracks?.length)return;
 const request=++motion.request;animationPause();
 $('motion-caption').textContent='Sampling motion…';
 try{const sample=await motionCommand('motion.sample',{character:motion.character,track:motion.track||motion.meta.tracks[0].id,rest:motion.rest,amount:motion.amount});if(request!==motion.request)return;motion.sample=sample;motion.time=Math.min(motion.time,sample.duration);$('motion-scrub').max=sample.duration;animationDraw();}catch(e){toast(String(e));}
}
async function animationRefresh(){if(state.view!=='animation')return;motion.meta=await motionCommand('motion.tracks',{character:motion.character});animationControls();await animationSample();}
function animationPause(){motion.playing=false;if($('motion-play')){$('motion-play').innerHTML=`<svg viewBox="0 0 24 24">${MOTION_ICONS.play}</svg>`;$('motion-play').title='Play motion';}}
async function animationPlay(){
 if(motion.playing){animationPause();return;}
 if(motion.rest){motion.rest=false;motion.track=motion.track||motion.meta?.tracks?.[0]?.id;animationControls();await animationSample();}
 if(!motion.sample)return;
 if(motion.time>=motion.sample.duration)motion.time=0;
 motion.playing=true;$('motion-play').innerHTML=`<svg viewBox="0 0 24 24">${MOTION_ICONS.pause}</svg>`;$('motion-play').title='Pause motion';
 let last=performance.now();function tick(now){if(!motion.playing)return;motion.time+=(now-last)/1000*motion.speed;last=now;if(motion.time>=motion.sample.duration){if(motion.sample.loop)motion.time%=motion.sample.duration;else{motion.time=motion.sample.duration;animationPause();}}animationDraw();if(motion.playing)requestAnimationFrame(tick);}requestAnimationFrame(tick);
}
function animationDraw(){
 const canvas=$('motion-canvas'),sample=motion.sample,meta=motion.meta;if(!canvas||!sample||!meta)return;
 const rect=canvas.parentElement.getBoundingClientRect();if(!rect.width||!rect.height)return;
 const dpr=devicePixelRatio||1,w=rect.width,h=rect.height;if(canvas.width!==Math.round(w*dpr)||canvas.height!==Math.round(h*dpr)){canvas.width=Math.round(w*dpr);canvas.height=Math.round(h*dpr);}
 const ctx=canvas.getContext('2d');ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,w,h);
 const b=sample.bounds||meta.bounds;const scale=(motion.fit?Math.min((w-64)/(b[2]-b[0]),(h-64)/(b[3]-b[1])):1)*motion.zoom;
 ctx.translate(w/2,h/2);ctx.scale(scale,scale);ctx.translate(-(b[0]+b[2])/2,-(b[1]+b[3])/2);
 const frame=sample.frames[Math.min(sample.frames.length-1,Math.floor(motion.time*sample.fps))];
 frame.layers.forEach((l,i)=>{if(!l.opacity)return;ctx.save();ctx.transform(...l.matrix);ctx.globalAlpha=l.opacity;const crop=meta.layers[i].crop;ctx.drawImage(motion.images[i],crop[0],crop[1]);ctx.restore();});
 $('motion-caption').textContent=`${motion.character} · ${motion.rest?'Rest pose':motion.track.replaceAll('-',' ')}`;
 $('motion-time').textContent=`${Math.floor(motion.time/60)}:${(motion.time%60).toFixed(3).padStart(6,'0')}`;$('motion-scrub').value=motion.time;
}
