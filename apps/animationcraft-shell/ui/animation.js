// The engine samples motion; this view only draws images using returned matrices.
const motion = { project:null, mode:localStorage.getItem('animation.mode')||'subject', character:null, meta:null, sample:null, images:[], time:0, playing:false, zoom:1, fit:true, track:'', rest:true, amount:100, speed:1, selected:null, loading:false, request:0, grid:localStorage.getItem("animation.grid")==="true", anchors:false, labels:false, pen:false, pathVisible:true, points:[], pathDuration:5, startSize:100, endSize:100, pathUndo:null };
function animationReset(){
 theatreRequireSaved();
 animationPause();
 Object.assign(motion,{project:null,character:null,meta:null,sample:null,images:[],assets:[],time:0,track:'',rest:true,selected:null,loading:false,request:motion.request+1,points:[],pathUndo:null,pen:false,fit:true,zoom:1});
 $('view-animation').replaceChildren();
}
const motionCommand = (id, params={}) => invoke('engine_command',{id,params});
const motionAssetUrl = path => `${convertFileSrc(path)}${String(convertFileSrc(path)).includes('?')?'&':'?'}v=${Date.now()}`;
const MOTION_ICONS = {
 pen:'<path d="m16 3 5 5-12 12-6 1 1-6zM14 5l5 5"/>', grid:'<path d="M3 3h18v18H3zM3 9h18M3 15h18M9 3v18M15 3v18"/>',
 play:'<path d="m8 5 11 7-11 7z"/>', pause:'<path d="M8 5v14M16 5v14"/>',
 reset:'<path d="M3 10a9 9 0 1 1 2 9M3 4v6h6"/>', fit:'<path d="M8 3H3v5M16 3h5v5M3 16v5h5M21 16v5h-5"/>',
 minus:'<path d="M5 12h14"/>', plus:'<path d="M5 12h14M12 5v14"/>', eye:'<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/>',
 layers:'<path d="m12 3 9 5-9 5-9-5zM3 13l9 5 9-5M3 18l9 5 9-5"/>', export:'<path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>', step:'<path d="m5 5 10 7-10 7zM19 5v14"/>'
};
function motionButton(id,label,icon,fn){return el('button',{id,class:'icon',title:label,'aria-label':label,html:`<svg viewBox="0 0 24 24">${MOTION_ICONS[icon]}</svg>`,onclick:fn});}
function motionSection(title,body,open=false,toneOverride=null){
 const groups={Action:['motion','play'],'Path & depth':['depth','pen'],'Motion & timing':['motion','step'],Layers:['layers','layers'],'Expression & pose':['pose','eye'],'Quality checks':['check','fit'],'Export & integrations':['export','export']};
 const [tone,icon]=groups[title]||[toneOverride||'layers','layers'];
 const heading=el('summary',{},el('span',{class:'motion-section-icon',html:`<svg viewBox="0 0 24 24">${MOTION_ICONS[icon]}</svg>`}),el('span',{},title),el('span',{class:'motion-section-chevron',html:ICONS.chev}));
 const node=el('details',{class:'motion-section','data-tone':tone},heading,el('div',{class:'motion-section-body'},body));node.open=open;return node;
}
function motionRange(id,label,min,max,step,value,unit,change){
 const out=el('output',{},`${Number(Number(value).toFixed(2))}${unit}`);
 const input=el('input',{id,type:'range',min,max,step,value,'aria-label':label,title:`${label}: ${min}–${max}${unit}`});
 const fill=()=>input.style.setProperty('--range-fill',`${100*(Number(input.value)-min)/(max-min)}%`);fill();
 input.addEventListener('input',()=>{out.textContent=`${input.value}${unit}`;fill();change(Number(input.value));});
 return el('label',{class:'motion-range'},el('span',{},label,out),input);
}
async function openAnimation(character){if(!theatreCanLeave())return false;motion.character=character;motion.selected=null;motion.project=null;await setView('animation');}
async function animationOpen(){
 if(motion.mode==='composite'){theatreRequireSaved();const view=$('view-animation');view.replaceChildren();sequenceAttach(view);return;}
 if(motion.project===state.project && motion.meta){animationDraw();return;}
 theatreRequireSaved();
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
function motionModeSwitch(){
 return el('div',{class:'motion-modes',role:'tablist','aria-label':'Animation mode'},
  ...['subject','composite'].map(m=>el('button',{
   class:`motion-mode${motion.mode===m?' active':''}`,id:`motion-mode-${m}`,role:'tab','data-mode':m,
   'aria-selected':String(motion.mode===m),title:m==='subject'?'A subject: its rig, parts and action tracks':'A composite: images arranged in time',
   onclick:async()=>{if(motion.mode===m||!theatreCanLeave())return;motion.mode=m;localStorage.setItem('animation.mode',m);animationPause();$('view-animation').replaceChildren();await animationOpen();},
  },m==='subject'?'Subject':'Composite')));
}
function animationShell(){
 theatreRequireSaved();
 const view=$('view-animation');view.replaceChildren();
 const inspector=el('aside',{class:'motion-inspector',id:'motion-inspector','aria-label':'Animation controls'});
 const stage=el('div',{class:'motion-stage',id:'motion-stage'},el('canvas',{id:'motion-canvas','aria-label':'Character animation canvas',tabindex:0}));
 const grid=motionButton('motion-grid','Show or hide grid','grid',()=>{motion.grid=!motion.grid;localStorage.setItem('animation.grid',String(motion.grid));grid.setAttribute('aria-pressed',String(motion.grid));animationDraw();});grid.setAttribute('aria-pressed',String(motion.grid));
 const toolbar=el('div',{class:'motion-stagebar'},motionModeSwitch(),el('span',{id:'motion-caption',class:'muted'},'Loading artwork…'),el('div',{class:'motion-tools'},
  motionButton('motion-pen','Pen · draw a motion path','pen',()=>{animationPause();motion.pen=!motion.pen;$('motion-pen').setAttribute('aria-pressed',String(motion.pen));$('motion-canvas').classList.toggle('drawing',motion.pen);$('motion-path-section')?.setAttribute('open','');}), motionButton('motion-path-visible','Show or hide the motion path','eye',()=>{motion.pathVisible=!motion.pathVisible;$('motion-path-visible').setAttribute('aria-pressed',String(motion.pathVisible));animationDraw();}), grid, motionButton('motion-fit','Fit artwork','fit',()=>{motion.fit=true;motion.zoom=1;animationDraw();}),
  el('button',{class:'icon',id:'motion-actual',title:'Actual size · 100%','aria-label':'Actual size',onclick:()=>{motion.fit=false;motion.zoom=1;animationDraw();}},'1:1'),
  motionButton('motion-minus','Zoom out','minus',()=>{motion.zoom/=1.2;animationDraw();}),motionButton('motion-plus','Zoom in','plus',()=>{motion.zoom*=1.2;animationDraw();})));
 const transport=el('div',{class:'motion-transport'},motionButton('motion-play','Play motion','play',animationPlay),motionButton('motion-rewind','Return to start','reset',()=>{animationPause();motion.time=0;animationDraw();}),motionButton('motion-step','Next frame','step',()=>{animationPause();motion.time=Math.min(motion.sample?.duration||0,motion.time+1/(motion.sample?.fps||30));animationDraw();}),el('span',{id:'motion-time',class:'motion-time'},'0:00.000'),el('input',{id:'motion-scrub',type:'range',min:0,max:1,step:0.001,value:0,'aria-label':'Scrub motion',oninput:e=>{animationPause();motion.time=Number(e.target.value);animationDraw();}}));
 const theatre=el('section',{class:'motion-theatre','aria-label':'Theatre timeline'});
 view.append(inspector,el('div',{class:'motion-main'},toolbar,stage,transport,theatre));
 theatreMount(theatre);
 new ResizeObserver(()=>animationDraw()).observe(stage);
 $('motion-canvas').addEventListener('click',()=>{if(!motion.pen)$('motion-layers')?.setAttribute('open','');});
 setupPathPen($('motion-canvas'));
 $('motion-canvas').addEventListener('keydown',e=>{if(e.key===' '){e.preventDefault();animationPlay();}});
}
async function animationLoad(){
 animationPause();const request=++motion.request;motion.loading=true;
 try{
  const meta=await motionCommand('motion.tracks',{character:motion.character});if(request!==motion.request)return;
  motion.meta=meta;if(!meta.layers?.some(part=>part.id===motion.selected))motion.selected=null;motion.track=meta.tracks?.[0]?.id||'';motion.time=0;
  motion.images=await Promise.all((meta.layers||[]).map(l=>new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(new Error(`Could not load ${l.label}`));img.src=motionAssetUrl(l.file);})));if(request!==motion.request)return;
  animationControls();await animationSample();
 }catch(e){animationEmpty('Artwork needs preparation',String(e));}finally{motion.loading=false;}
}
function animationControls(){
 const box=$('motion-inspector');box.replaceChildren();const meta=motion.meta;
 const picker=el('select',{id:'motion-object','aria-label':'Animated object',title:'Choose an object',onchange:async e=>{motion.character=e.target.value;motion.selected=null;updateMagicSelection();await animationLoad();}},(motion.assets||[]).map(a=>el('option',{value:a.id},a.name||a.id)));picker.value=motion.character;
 box.append(el('div',{class:'motion-object-head'},el('span',{class:'dot ok',title:'Artwork loaded'}),picker));
 updateMagicSelection();
 const tracks=el('select',{id:'motion-track','aria-label':'Motion track',title:'Choose a motion track',onchange:async e=>{if(!e.target.value)return;motion.track=e.target.value;acFeedback.report(null,null,{stage:'clip_switched',clip:motion.track});motion.rest=!e.target.value;motion.time=0;const selected=meta.tracks.find(t=>t.id===motion.track);if(selected?.path?.points){motion.points=selected.path.points;motion.pathDuration=selected.duration;motion.startSize=selected.path.start_size;motion.endSize=selected.path.end_size;animationControls();}await animationSample();}},el('option',{value:''},'Choose a custom track…'),(meta.tracks||[]).filter(t=>!t.standard).map(t=>el('option',{value:t.id},t.id.replace(/^.*?-beat\d+-/, '').replaceAll('-',' '))));tracks.value=motion.rest||motion.track.startsWith('@')?'':motion.track;
 const actions=el('div',{class:'motion-actions-grid'},(meta.tracks||[]).filter(t=>t.standard).map(t=>el('button',{id:`motion-action-${t.id.slice(1)}`,class:'motion-action','aria-pressed':String(!motion.rest&&motion.track===t.id),title:`${t.label} · scripted whole-object motion`,onclick:async()=>{motion.track=t.id;acFeedback.report(null,null,{stage:'clip_switched',clip:t.id});motion.rest=false;motion.time=0;animationControls();await animationSample();await animationPlay();}},t.label)),el('button',{id:'motion-action-rest',class:'motion-action','aria-pressed':String(motion.rest),title:'Rest pose',onclick:async()=>{motion.rest=true;motion.time=0;animationControls();await animationSample();}},'Rest'));
 box.append(motionSection('Action',[actions,el('span',{class:'group-label'},'Custom tracks'),tracks],true));
 box.append(pathControls());
 box.append(motionSection('Motion & timing',[
 motionRange('motion-speed','Playback speed',0.25,2.25,0.05,motion.speed,'×',v=>motion.speed=v),
 motionRange('motion-amount','Motion amount',0,100,1,motion.amount,'%',v=>{motion.amount=v;clearTimeout(motion.amountTimer);motion.amountTimer=setTimeout(animationSample,140);})],true));
 const layers=el('div',{class:'motion-layers'});
 for(const l of [...meta.layers].reverse()){
  const row=el('div',{class:`motion-layer${motion.selected===l.id?' selected':''}`,id:`motion-layer-${l.id}`,title:l.id});const shown=l.state?.visible!==false;
  row.append(el('img',{src:motionAssetUrl(l.file),alt:''}),el('button',{class:'motion-layer-name','aria-pressed':String(motion.selected===l.id),'aria-label':`Select part ${l.label} (${l.id})`,onclick:()=>{motion.selected=l.id;animationControls();animationDraw();updateMagicSelection();}},l.label),motionButton(`motion-visible-${l.id}`,`${shown?'Hide':'Show'} ${l.label}`,'eye',async()=>{await motionCommand('part.set_state',{part:l.id,visible:!shown});await animationRefresh();}));
  row.lastChild.setAttribute('aria-pressed',String(shown));if(!shown)row.classList.add('hidden-layer');layers.append(row);
 }
 const section=motionSection('Layers',layers,true);section.id='motion-layers';
 const selected=meta.layers.find(part=>part.id===motion.selected);
 const candidate=selected&&typeof imageCandidateForPart==='function'?imageCandidateForPart(motion.character,selected.id):null;
 const compare=selected?el('div',{class:'magic-part-compare',id:'magic-part-compare'},
  el('div',{class:'magic-compare-title'},`${selected.label||selected.id} · ${selected.id}`),
  el('div',{class:'magic-compare-grid'},
   el('figure',{class:'magic-compare-frame'},el('img',{src:motionAssetUrl(selected.file),alt:`Original ${selected.label||selected.id}`}),el('figcaption',{},'Original')),
   el('figure',{class:'magic-compare-frame'},candidate?el('img',{src:convertFileSrc(candidate.image),alt:`Candidate for ${selected.label||selected.id}` }):el('div',{class:'magic-compare-empty'},'Generate a candidate for this selected part.'),el('figcaption',{},candidate?'Candidate · needs review':'Candidate'))
  ),
  candidate?el('p',{class:'why'},'Original source remains unchanged until you explicitly replace it. Candidate dimensions must match the selected part exactly; motion, pivot, and timing stay in place.'):null
 ):el('p',{class:'why'},'Select a layer to compare its original with a generated candidate.');
 if(selected){
  const sourceSize=selected.source_size||[];const dimensionsMatch=!!candidate&&candidate.result.width===sourceSize[0]&&candidate.result.height===sourceSize[1];
  const actionStatus=el('p',{class:'why',id:'part-image-status'},dimensionsMatch?'Candidate dimensions match the selected part.':candidate?`Cannot replace: candidate is ${candidate.result.width}×${candidate.result.height}; ${selected.label||selected.id} requires ${sourceSize[0]||'?'}×${sourceSize[1]||'?'}.`: 'Replacement requires a reviewed candidate for this captured part.');
  const undoKey=`magic.part-image-undo.${state.project}.${motion.character}.${selected.id}`;
  const savedUndo=localStorage.getItem(undoKey);
  const replaceButton=el('button',{class:'start',id:'part-image-replace','aria-label':`Replace ${selected.label||selected.id} with reviewed candidate`,title:!dimensionsMatch?'Candidate must have the same pixel dimensions as this part':'Replace only this captured part; motion and pivot are preserved',onclick:async()=>{
   replaceButton.disabled=true;actionStatus.textContent='Replacing selected part…';
   try{const result=await motionCommand('part.replace_image',{asset:candidate.assetId,part:candidate.partId,candidate:candidate.image,expected_rig_sha256:candidate.rigSha256,expected_part_sha256:candidate.partSha256,expected_candidate_sha256:candidate.result.sha256,expected_revision:candidate.revision});
    localStorage.setItem(undoKey,result.undo);actionStatus.textContent='Part replaced. Undo is available for this latest image change.';await animationRefresh();
   }catch(error){actionStatus.textContent=String(error);replaceButton.disabled=false;}
  }},'Replace selected part');
  replaceButton.disabled=!dimensionsMatch||!candidate.rigSha256||!candidate.partSha256||!Number.isSafeInteger(candidate.revision)||candidate.revision!==meta.image_revision;
  const undoButton=el('button',{class:'ghost',id:'part-image-undo','aria-label':`Undo image replacement for ${selected.label||selected.id}`,onclick:async()=>{
   undoButton.disabled=true;actionStatus.textContent='Restoring original part image…';
   try{await motionCommand('part.undo_image',{asset:motion.character,undo:savedUndo});localStorage.removeItem(undoKey);actionStatus.textContent='Original part image restored.';await animationRefresh();}
   catch(error){actionStatus.textContent=String(error);undoButton.disabled=false;}
  }},'Undo image replacement');
  undoButton.disabled=!savedUndo;
  compare.append(actionStatus,replaceButton,undoButton);
 }
 const assetRows=(motion.assets||[]).map(asset=>el('button',{class:`magic-asset-row${asset.id===motion.character?' active':''}`,id:`magic-asset-${asset.id}`,title:`Show parts for ${asset.name||asset.id}`,onclick:()=>{if(asset.id!==motion.character)openAnimation(asset.id);}},el('span',{},asset.name||asset.id),el('span',{class:'muted'},asset.id)));
 const assetsSection=motionSection('Project assets',assetRows.length?assetRows:el('p',{class:'why'},'No project assets are available yet.'));
 const partsSlot=$('magic-parts-slot');
 if(partsSlot)partsSlot.replaceChildren(assetsSection,section,compare);else box.append(section);
 const controls=meta.controls?.joints||[];
 box.append(motionSection('Expression & pose',el('p',{class:'muted'},controls.length?'This rig exposes joint bindings.':'Standard actions move the whole object. Custom tracks add detailed acting to its layers.')));
 box.append(motionSection('Quality checks',el('p',{class:'muted'},`${meta.layers.length} layers · ${meta.tracks.length} motion tracks`)));
 const exportStatus=el('div',{class:'why'});
 const exportButton=el('button',{class:'start',id:'motion-export',title:'Export this action as a transparent video track',onclick:async()=>{
  exportButton.disabled=true;exportStatus.textContent='Rendering transparent track…';
  try{const result=await motionCommand('motion.export_track',{character:motion.character,track:motion.track,amount:motion.amount,duration:motion.sample.duration/motion.speed});
   const file=result.outputs?.[0]?.file;exportStatus.replaceChildren(el('span',{},'Track ready'),file?fileButton({path:file,name:file.split('/').pop()}):null);
  }catch(error){exportStatus.textContent=String(error);}finally{exportButton.disabled=false;}
 }},'Export track');
 box.append(motionSection('Export & integrations',[exportButton,exportStatus]));
}
async function animationSample(){
 if(!motion.meta?.tracks?.length)return;
 const request=++motion.request;animationPause();
 $('motion-caption').textContent='Sampling motion…';
 try{const sample=await motionCommand('motion.sample',{character:motion.character,track:motion.track||motion.meta.tracks[0].id,rest:motion.rest,amount:motion.amount});if(request!==motion.request)return;motion.sample=sample;motion.time=Math.min(motion.time,sample.duration);$('motion-scrub').max=sample.duration;animationDraw();}catch(e){toast(String(e));}
}
async function animationRefresh(){if(state.view!=='animation')return;motion.meta=await motionCommand('motion.tracks',{character:motion.character});motion.images=await Promise.all((motion.meta.layers||[]).map(l=>new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(new Error(`Could not refresh ${l.label}`));img.src=motionAssetUrl(l.file);})));animationControls();await animationSample();}
function animationPause(){
 if(motion.mode==='composite'){seqPause();return;}motion.playing=false;if($('motion-play')){$('motion-play').innerHTML=`<svg viewBox="0 0 24 24">${MOTION_ICONS.play}</svg>`;$('motion-play').title='Play motion';}}
async function animationPlay(){
 if(motion.mode==='composite'){seqPlay();return;}
 if(motion.playing){animationPause();return;}
 if(motion.rest){motion.rest=false;motion.track=motion.track||motion.meta?.tracks?.[0]?.id;animationControls();await animationSample();}
 if(!motion.sample)return;
 if(motion.time>=motion.sample.duration)motion.time=0;
 motion.playing=true;$('motion-play').innerHTML=`<svg viewBox="0 0 24 24">${MOTION_ICONS.pause}</svg>`;$('motion-play').title='Pause motion';
 let last=performance.now();function tick(now){if(!motion.playing)return;motion.time+=(now-last)/1000*motion.speed;last=now;if(motion.time>=motion.sample.duration){if(motion.sample.loop)motion.time%=motion.sample.duration;else{motion.time=motion.sample.duration;animationPause();}}animationDraw();if(motion.playing)requestAnimationFrame(tick);}requestAnimationFrame(tick);
}
function animationDraw(){
 if(motion.mode==='composite'){seqDraw();return;}
 const canvas=$('motion-canvas'),sample=motion.sample,meta=motion.meta;if(!canvas||!sample||!meta)return;
 const rect=canvas.parentElement.getBoundingClientRect();if(!rect.width||!rect.height)return;
 const dpr=devicePixelRatio||1,w=rect.width,h=rect.height;if(canvas.width!==Math.round(w*dpr)||canvas.height!==Math.round(h*dpr)){canvas.width=Math.round(w*dpr);canvas.height=Math.round(h*dpr);}
 const ctx=canvas.getContext('2d');ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,w,h);
 const b=sample.bounds||meta.bounds;const scale=(motion.fit?Math.min((w-64)/(b[2]-b[0]),(h-64)/(b[3]-b[1])):1)*motion.zoom;
 motion.viewTransform={scale,cx:(b[0]+b[2])/2,cy:(b[1]+b[3])/2,w,h};
 ctx.translate(w/2,h/2);ctx.scale(scale,scale);ctx.translate(-(b[0]+b[2])/2,-(b[1]+b[3])/2);
 if(motion.grid){const gap=50;ctx.save();ctx.strokeStyle=getComputedStyle(document.documentElement).getPropertyValue('--line');ctx.lineWidth=1/scale;ctx.beginPath();const left=(b[0]+b[2])/2-w/(2*scale),top=(b[1]+b[3])/2-h/(2*scale);for(let x=Math.floor(left/gap)*gap;x<left+w/scale;x+=gap){ctx.moveTo(x,top);ctx.lineTo(x,top+h/scale);}for(let y=Math.floor(top/gap)*gap;y<top+h/scale;y+=gap){ctx.moveTo(left,y);ctx.lineTo(left+w/scale,y);}ctx.stroke();ctx.restore();}
 const frame=sample.frames[Math.min(sample.frames.length-1,Math.floor(motion.time*sample.fps))];
 frame.layers.forEach((l,i)=>{if(!l.opacity)return;ctx.save();ctx.transform(...l.matrix);theatreTransform(ctx,meta.layers[i]);ctx.globalAlpha=l.opacity;const crop=meta.layers[i].crop;ctx.drawImage(motion.images[i],crop[0],crop[1]);ctx.restore();});
 drawMotionPath(ctx,scale);
 $('motion-caption').textContent=`${motion.character} · ${motion.rest?'Rest pose':(meta.tracks.find(t=>t.id===motion.track)?.label||motion.track.replaceAll('-',' '))}`;
 $('motion-time').textContent=`${Math.floor(motion.time/60)}:${(motion.time%60).toFixed(3).padStart(6,'0')}`;$('motion-scrub').value=motion.time;
 theatreSeek(motion.time);
}

function pathControls(){
 const apply=el('button',{id:'motion-path-apply',class:'start',onclick:applyMotionPath},'Apply path');apply.disabled=motion.points.length<2;
 const undo=motionButton('motion-path-undo','Undo the last drawn path','reset',async()=>{if(!motion.pathUndo)return;try{await motionCommand('motion.undo_path',{character:motion.character,undo:motion.pathUndo});motion.pathUndo=null;motion.points=[];await animationLoad();}catch(e){toast(String(e));}});undo.disabled=!motion.pathUndo;
 const section=motionSection('Path & depth',[
  el('p',{class:'motion-help',title:'Select Pen above the stage, then draw. Playback moves from the first point at zero to the last point at the chosen duration.'},'Draw a route. Set its timing.'),
  motionRange('motion-path-duration','Duration',0.1,Math.max(60,motion.pathDuration),0.1,motion.pathDuration,'s',v=>{motion.pathDuration=v;animationDraw();}),
  motionRange('motion-size-start','Start size',1,Math.max(200,motion.startSize),1,motion.startSize,'%',v=>motion.startSize=v),
  motionRange('motion-size-end','End size',1,Math.max(200,motion.endSize),1,motion.endSize,'%',v=>motion.endSize=v),
  el('div',{class:'motion-path-actions'},motionButton('motion-path-clear','Clear the drawn line','reset',()=>{motion.points=[];$('motion-path-apply').disabled=true;animationDraw();}),undo,apply)
 ],false);section.id='motion-path-section';return section;
}
function setupPathPen(canvas){
 let drawing=false;
 const point=e=>{const v=motion.viewTransform;if(!v)return;const r=canvas.getBoundingClientRect();const p=[(e.clientX-r.left-v.w/2)/v.scale+v.cx,(e.clientY-r.top-v.h/2)/v.scale+v.cy];const last=motion.points.at(-1);if(!last||Math.hypot(p[0]-last[0],p[1]-last[1])*v.scale>4){if(motion.points.length<512)motion.points.push(p);$('motion-path-apply').disabled=motion.points.length<2;animationDraw();}};
 canvas.addEventListener('pointerdown',e=>{if(!motion.pen)return;e.preventDefault();canvas.setPointerCapture(e.pointerId);drawing=true;point(e);});
 canvas.addEventListener('pointermove',e=>{if(drawing)point(e);});
 canvas.addEventListener('pointerup',e=>{if(drawing){point(e);drawing=false;}});
 canvas.addEventListener('pointercancel',()=>drawing=false);
}
function drawMotionPath(ctx,scale){
 if(!motion.pathVisible||!motion.points.length)return;ctx.save();ctx.strokeStyle=getComputedStyle(document.documentElement).getPropertyValue('--accent');ctx.fillStyle=ctx.strokeStyle;ctx.lineWidth=1.5/scale;ctx.beginPath();motion.points.forEach((p,i)=>i?ctx.lineTo(...p):ctx.moveTo(...p));ctx.stroke();
 for(const i of [0,motion.points.length-1]){const p=motion.points[i];ctx.beginPath();ctx.arc(...p,4/scale,0,Math.PI*2);ctx.fill();ctx.font=`${12/scale}px system-ui`;ctx.fillText(i===0?'0':`${motion.pathDuration}s`,p[0]+8/scale,p[1]-8/scale);}ctx.restore();
}
async function applyMotionPath(){
 animationPause();const button=$('motion-path-apply');button.disabled=true;
 try{const result=await motionCommand('motion.path',{character:motion.character,track:motion.track||motion.meta.tracks[0].id,points:motion.points,duration:motion.pathDuration,start_size:motion.startSize,end_size:motion.endSize});
 motion.pathUndo=result.undo;motion.track=result.track;motion.rest=false;motion.pen=false;motion.time=0;motion.meta=await motionCommand('motion.tracks',{character:motion.character});animationControls();$('motion-pen').setAttribute('aria-pressed','false');$('motion-canvas').classList.remove('drawing');await animationSample();toast('Path ready · play to review');
 }catch(e){toast(String(e));}finally{if($('motion-path-apply'))$('motion-path-apply').disabled=motion.points.length<2;}
}
