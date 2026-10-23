// The engine samples motion; this view only draws images using returned matrices.
const motion = { project:null, mode:localStorage.getItem('animation.backendMode')==='composite'?'composite':'subject', character:null, meta:null, sample:null, images:[], time:0, playing:false, tick:0, playId:0, resizeObserver:null, zoom:1, fit:true, track:'', rest:true, amount:100, speed:1, selected:null, partsTab:'compare', photoDrafts:{}, openSections:(()=>{try{return JSON.parse(localStorage.getItem('animation.openSections')||'{}')||{};}catch{return {};}})(), loading:false, request:0, grid:localStorage.getItem("animation.grid")==="true", anchors:false, labels:false, pen:false, pathVisible:true, points:[], pathDuration:5, startSize:100, endSize:100, pathUndo:null, pathLayer:null, previewLoop:null };
function animationFeedback(){
 return {backend:motion.mode,character:motion.character,track:motion.track,rest:motion.rest,time:motion.time,playing:motion.playing,selected_layer:motion.selected,speed:motion.speed,amount:motion.amount,duration:motion.sample?.duration??null,loop:motion.sample?.loop??null,rig_sha256:motion.meta?.rig_sha256??null,layer_order:motion.meta?.layer_order||null,selected_layer_depth:motion.meta?.layers?.find(l=>l.id===motion.selected)?.depth??null,open_sections:{...motion.openSections},refresh_error:motion.refreshError||null,last_edit:motion.lastEdit?.project===state.project?motion.lastEdit:null,last_pose:motion.lastPose?.project===state.project?motion.lastPose:null};
}
// UI view preferences are project-scoped; rig/Theatre persistence remains owned by the engine.
function animationRemember(){
 if(!state.project||motion.project!==state.project||!motion.character)return;
 const value={project:state.project,character:motion.character,track:motion.track,selected:motion.selected,rest:motion.rest,time:motion.time,speed:motion.speed,amount:motion.amount,previewLoop:motion.previewLoop,points:motion.points.map(p=>[...p]),pathUndo:motion.pathUndo,pathLayer:motion.pathLayer,pathDuration:motion.pathDuration,startSize:motion.startSize,endSize:motion.endSize};
 const serialized=JSON.stringify(value);if(motion.remembered===serialized)return;motion.remembered=serialized;
 try{const prior=JSON.parse(localStorage.getItem('animation.sessions')||'[]');localStorage.setItem('animation.sessions',JSON.stringify([value,...(Array.isArray(prior)?prior:[]).filter(v=>v.project!==state.project)].slice(0,20)));}catch{}
}
function animationRestore(){
 try{const entries=JSON.parse(localStorage.getItem('animation.sessions')||'[]'),v=Array.isArray(entries)?entries.find(v=>v.project===state.project):null;if(!v||typeof v.character!=='string'||typeof v.track!=='string')return null;
 const number=(key,fallback,min,max)=>Number.isFinite(v[key])&&v[key]>=min&&v[key]<=max?v[key]:fallback;
 return {character:v.character,track:v.track,selected:typeof v.selected==='string'?v.selected:null,rest:v.rest===true,time:number('time',0,0,600),speed:number('speed',1,0.25,2.25),amount:number('amount',100,0,100),previewLoop:typeof v.previewLoop==='boolean'?v.previewLoop:null,points:Array.isArray(v.points)&&v.points.length<=512&&v.points.every(p=>Array.isArray(p)&&p.length===2&&p.every(Number.isFinite))?v.points.map(p=>[...p]):[],pathUndo:typeof v.pathUndo==='string'?v.pathUndo:null,pathLayer:typeof v.pathLayer==='string'?v.pathLayer:null,pathDuration:number('pathDuration',5,0.1,600),startSize:number('startSize',100,1,1000),endSize:number('endSize',100,1,1000)};
 }catch{return null;}
}
function animationReset(){
 theatreRequireSaved();animationRemember();
 blenderDispose();
 animationPause();
 Object.assign(motion,{project:null,character:null,meta:null,sample:null,images:[],assets:[],time:0,track:'',rest:true,selected:null,photoDrafts:{},loading:false,request:motion.request+1,points:[],pathUndo:null,pathLayer:null,previewLoop:null,pen:false,fit:true,zoom:1});
 $('view-animation').replaceChildren();
}
const motionCommand = (id, params={}) => invoke('engine_command',{id,params});
const motionAssetUrl = path => `${convertFileSrc(path)}${String(convertFileSrc(path)).includes('?')?'&':'?'}v=${motion.meta?.image_revision??0}`;
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
 const node=el('details',{class:'motion-section','data-tone':tone},heading,el('div',{class:'motion-section-body'},body));
 node.open=Object.hasOwn(motion.openSections,title)?motion.openSections[title]===true:open;
 node.addEventListener('toggle',()=>{
  motion.openSections[title]=node.open;
  localStorage.setItem('animation.openSections',JSON.stringify(motion.openSections));
 });
 return node;
}
function motionRange(id,label,min,max,step,value,unit,change){
 const out=el('output',{},`${Number(Number(value).toFixed(2))}${unit}`);
 const input=el('input',{id,type:'range',min,max,step,value,'aria-label':label,title:`${label}: ${min}–${max}${unit}`});
 const fill=()=>input.style.setProperty('--range-fill',`${100*(Number(input.value)-min)/(max-min)}%`);fill();
 input.addEventListener('input',()=>{out.textContent=`${input.value}${unit}`;fill();change(Number(input.value));});
 return el('label',{class:'motion-range'},el('span',{},label,out),input);
}
async function openAnimation(character){if(!theatreCanLeave())return false;motion.character=character;motion.selected=null;motion.project=null;motion.explicitCharacter=true;await setView('animation');}
async function animationOpen(){
 if(motion.mode==='composite'){theatreRequireSaved();const view=$('view-animation');view.replaceChildren();sequenceAttach(view);return;}
 if(motion.project===state.project && motion.meta){animationDraw();return;}
 theatreRequireSaved();
 motion.project=state.project;motion.meta=null;motion.sample=null;motion.time=0;motion.rest=true;
 motion.resume=motion.explicitCharacter?null:animationRestore();motion.explicitCharacter=false;if(motion.resume)Object.assign(motion,motion.resume);
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
 return el('div',{class:'motion-modes',role:'group','aria-label':'Animation authoring and render tools'},
  ...['subject','composite'].map(m=>el('button',{
   class:`motion-mode${motion.mode===m?' active':''}`,id:`motion-mode-${m}`,'data-mode':m,
   'aria-pressed':String(motion.mode===m),title:m==='subject'?'Subject authoring, parts and timeline':'Images arranged in time',
   onclick:async()=>{if(motion.mode===m||!theatreCanLeave())return;blenderDispose();motion.project=null;motion.meta=null;motion.request++;motion.mode=m;localStorage.setItem('animation.backendMode',m);animationPause();$('view-animation').replaceChildren();await animationOpen();},
  },m==='subject'?'Subject':'Composite')),
  el('button',{class:'motion-mode',id:'motion-mode-blender',title:'Open Blender rendering alongside this editor',onclick:async()=>{try{await blenderAnimationOpen();}catch(error){toast(String(error));}}},'Blender render'),el('button',{class:'motion-mode',id:'motion-mode-native',title:'Saved Synfig sources and OpenToonz availability',onclick:()=>nativeAnimationOpen().catch(error=>toast(String(error)))},'Native tools'));
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
 const transport=el('div',{class:'motion-transport'},motionButton('motion-play','Play motion','play',animationPlay),motionButton('motion-rewind','Return to start','reset',()=>{animationPause();motion.time=0;animationDraw();}),motionButton('motion-step-back','Previous frame','step',()=>animationStep(-1)),motionButton('motion-step','Next frame','step',()=>animationStep(1)),el('span',{id:'motion-time',class:'motion-time'},'0:00.000'),el('input',{id:'motion-scrub',type:'range',min:0,max:1,step:0.001,value:0,'aria-label':'Scrub motion',oninput:e=>animationSeek(Number(e.target.value))}));
 const partBar=el('div',{class:'motion-part-bar',id:'motion-part-bar',hidden:''});
 view.append(inspector,el('div',{class:'motion-main'},toolbar,stage,transport,partBar));
 if(motion.mode!=='composite'){
  const theatre=el('section',{class:'motion-theatre','aria-label':'Theatre timeline'});
  view.append(theatre);
  theatreMount(theatre);
 }
 motion.resizeObserver?.disconnect();motion.resizeObserver=new ResizeObserver(()=>animationDraw());motion.resizeObserver.observe(stage);
 $('motion-canvas').addEventListener('click',()=>{if(!motion.pen)$('motion-layers')?.setAttribute('open','');});
 setupPathPen($('motion-canvas'));
 $('motion-canvas').addEventListener('keydown',e=>{if(e.key===' '){e.preventDefault();animationPlay();}});
}
async function animationLoad(){
 animationPause();const request=++motion.request,resume=motion.resume;motion.resume=null;motion.loading=true;
 try{
  const meta=await motionCommand('motion.tracks',{character:motion.character});if(request!==motion.request)return;
  motion.meta=meta;motion.sample=null;motion.photoDrafts={};if(!meta.layers?.some(part=>part.id===motion.selected))motion.selected=null;motion.track=resume&&meta.tracks?.some(t=>t.id===resume.track)?resume.track:meta.tracks?.[0]?.id||'';motion.time=resume?.time||0;
  motion.images=await Promise.all((meta.layers||[]).map(l=>new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(new Error(`Could not load ${l.label}`));img.src=motionAssetUrl(l.file);})));if(request!==motion.request)return;
  // Open moving: the rig's first authored clip, else the standard Sway, so its parts animate at once.
  const opening=!resume&&motion.rest&&((meta.tracks||[]).find(t=>!t.standard)||(meta.tracks||[]).find(t=>t.id==='@sway'));
  if(opening){motion.track=opening.id;motion.rest=false;}
  animationControls();await animationSample();refreshPartDrafts();
  if(opening&&motion.meta===meta&&!motion.playing)await animationPlay();
 }catch(e){animationEmpty('Artwork needs preparation',String(e));}finally{motion.loading=false;}
}
function animationControls(){
 const box=$('motion-inspector');box.replaceChildren();const meta=motion.meta;
 const authoring=motionAuthoringPanels(meta);
 const picker=el('select',{id:'motion-object','aria-label':'Animated object',title:'Choose an object',onchange:async e=>{if(!theatreCanLeave()){e.target.value=motion.character;return;}motion.character=e.target.value;motion.selected=null;updateMagicSelection();await animationLoad();}},(motion.assets||[]).map(a=>el('option',{value:a.id},a.name||a.id)));picker.value=motion.character;
 box.append(el('div',{class:'motion-object-head'},el('span',{class:'dot ok',title:'Artwork loaded'}),picker),authoring.status);
 updateMagicSelection();
 const tracks=el('select',{id:'motion-track','aria-label':'Motion track',title:'Choose a motion track',onchange:async e=>{if(!e.target.value)return;if(!theatreCanLeave()){e.target.value=motion.rest||motion.track.startsWith('@')?'':motion.track;return;}animationPause();motion.track=e.target.value;motion.previewLoop=null;acFeedback.report(null,null,{stage:'clip_switched',clip:motion.track});motion.rest=!e.target.value;motion.time=0;const selected=meta.tracks.find(t=>t.id===motion.track);motion.points=[];motion.pathLayer=null;if(selected?.path?.points){motion.points=selected.path.points.map(p=>[...p]);motion.pathLayer=selected.path.layer;motion.pathDuration=selected.duration;motion.startSize=selected.path.start_size;motion.endSize=selected.path.end_size;}animationControls();await animationSample();}},el('option',{value:''},'Choose a custom track…'),(meta.tracks||[]).filter(t=>!t.standard).map(t=>el('option',{value:t.id},t.label||t.id.replaceAll('-',' '))));tracks.value=motion.rest||motion.track.startsWith('@')?'':motion.track;
 const actions=el('div',{class:'motion-actions-grid'},(meta.tracks||[]).filter(t=>t.standard).map(t=>el('button',{id:`motion-action-${t.id.slice(1)}`,class:'motion-action','aria-pressed':String(!motion.rest&&motion.track===t.id),title:`${t.label} · scripted whole-object motion`,onclick:async()=>{if(!theatreCanLeave())return;animationPause();motion.track=t.id;motion.previewLoop=null;acFeedback.report(null,null,{stage:'clip_switched',clip:t.id});motion.rest=false;motion.time=0;animationControls();await animationSample();await animationPlay();}},t.label)),el('button',{id:'motion-action-rest',class:'motion-action','aria-pressed':String(motion.rest),title:'Rest pose',onclick:async()=>{if(!theatreCanLeave())return;animationPause();motion.rest=true;motion.time=0;animationControls();await animationSample();}},'Rest'));
 box.append(motionSection('Action',[actions,el('span',{class:'group-label'},'Custom tracks'),tracks,authoring.action],true));
 box.append(pathControls());
 const loop=el('input',{id:'motion-preview-loop',type:'checkbox',onchange:e=>{motion.previewLoop=e.target.checked;e.target.defaultChecked=e.target.checked;animationRemember();}});loop.checked=motion.previewLoop??meta.tracks.find(t=>t.id===motion.track)?.loop??false;loop.defaultChecked=loop.checked;
 box.append(motionSection('Motion & timing',[el('label',{},loop,'Loop preview'),
 motionRange('motion-speed','Playback speed',0.25,2.25,0.05,motion.speed,'×',v=>{motion.speed=v;animationRemember();}),
 motionRange('motion-amount','Motion amount',0,100,1,motion.amount,'%',v=>{motion.amount=v;animationRemember();clearTimeout(motion.amountTimer);motion.amountTimer=setTimeout(animationSample,140);})],false));
 const layers=el('div',{class:'motion-layers'});
 for(const l of [...meta.layers].reverse()){
  const row=el('div',{class:`motion-layer${motion.selected===l.id?' selected':''}`,id:`motion-layer-${l.id}`,title:l.id});const shown=l.state?.visible!==false;
  row.append(el('img',{src:motionAssetUrl(l.file),alt:''}),el('button',{class:'motion-layer-name','aria-pressed':String(motion.selected===l.id),'aria-label':`Select part ${l.label} (${l.id})`,onclick:()=>{motion.selected=l.id;animationControls();animationDraw();updateMagicSelection();}},l.label),motionButton(`motion-visible-${l.id}`,`${shown?'Hide':'Show'} ${l.label}`,'eye',async()=>{await motionCommand('part.set_state',{expected_project:state.project,character:motion.character,part:l.id,visible:!shown});await animationRefresh();}));
  row.lastChild.setAttribute('aria-pressed',String(shown));if(!shown)row.classList.add('hidden-layer');layers.append(row);
 }
 const section=motionSection('Layers',authoring.layers,true);section.id='motion-layers';
 const selected=meta.layers.find(part=>part.id===motion.selected);
 const candidate=selected?partCandidate(selected.id):null;
 const partsStrip=el('div',{class:'magic-parts-strip','aria-label':'Parts strip'},...(meta.layers||[]).map(part=>el('button',{class:`magic-part-thumb${motion.selected===part.id?' active':''}`,id:`magic-thumb-${part.id}`,title:`Select ${part.label||part.id} (${part.id})`,onclick:()=>{motion.selected=part.id;animationControls();animationDraw();updateMagicSelection();}},el('img',{src:motionAssetUrl(part.file),alt:''}),el('span',{},part.id))));
 const compare=el('div',{class:'magic-part-compare',id:'magic-part-compare'},partsStrip);
 if(!selected)compare.append(el('p',{class:'why'},'Select a part to compare its original with a candidate.'));
 else{
  const name=selected.label||selected.id,sourceSize=selected.source_size||[];
  const size=(w,h)=>w&&h?` · ${w}×${h}`:'';
  const dimensionsMatch=!!candidate&&candidate.result.width===sourceSize[0]&&candidate.result.height===sourceSize[1];
  compare.append(el('div',{class:'magic-compare-grid'},
   el('figure',{class:'magic-compare-frame'},el('img',{src:motionAssetUrl(selected.file),alt:`Original ${name}`}),el('figcaption',{},`Original${size(sourceSize[0],sourceSize[1])}`)),
   el('figure',{class:'magic-compare-frame'},candidate?el('img',{src:candidateUrl(candidate),alt:`Candidate for ${name}`}):el('div',{class:'magic-compare-empty'},'No candidate yet'),
    el('figcaption',{},candidate?`${candidate.source==='photocraft'?'PhotoCraft draft':'Candidate'}${size(candidate.result.width,candidate.result.height)}`:'Candidate'))));
  const statusText=dimensionsMatch?'Sizes match · the original stays until you replace it':candidate?`Cannot replace: candidate is ${candidate.result.width}×${candidate.result.height}; ${name} needs ${sourceSize[0]||'?'}×${sourceSize[1]||'?'}`:'Generate a candidate or save an edit in PhotoCraft';
  const actionStatus=el('p',{class:'magic-part-status',id:'part-image-status',title:statusText,role:'status'},statusText);
  const undoKey=`magic.part-image-undo.${state.project}.${motion.character}.${selected.id}`;
  const savedUndo=localStorage.getItem(undoKey);
  const replaceButton=el('button',{class:'start',id:'part-image-replace','aria-label':`Replace ${name} with reviewed candidate`,title:!dimensionsMatch?'Candidate must have the same pixel dimensions as this part':'Replace only this captured part; motion and pivot are preserved',onclick:async()=>{
   replaceButton.disabled=true;actionStatus.textContent='Replacing selected part…';
   try{const result=await motionCommand('part.replace_image',{asset:candidate.assetId,part:candidate.partId,candidate:candidate.image,expected_rig_sha256:candidate.rigSha256,expected_part_sha256:candidate.partSha256,expected_candidate_sha256:candidate.result.sha256,expected_revision:candidate.revision});
    localStorage.setItem(undoKey,result.undo);actionStatus.textContent='Part replaced · Undo restores the previous image';await animationRefresh();
   }catch(error){actionStatus.textContent=String(error);replaceButton.disabled=false;}
  }},'Replace');
  replaceButton.disabled=!dimensionsMatch||!candidate.rigSha256||!candidate.partSha256||!Number.isSafeInteger(candidate.revision)||candidate.revision!==meta.image_revision;
  const undoButton=el('button',{class:'ghost',id:'part-image-undo','aria-label':`Undo image replacement for ${name}`,title:'Undo the latest image replacement',onclick:async()=>{
   undoButton.disabled=true;actionStatus.textContent='Restoring original part image…';
   try{await motionCommand('part.undo_image',{asset:motion.character,undo:savedUndo});localStorage.removeItem(undoKey);actionStatus.textContent='Original part image restored';await animationRefresh();}
   catch(error){actionStatus.textContent=String(error);undoButton.disabled=false;}
  }},'Undo');
  undoButton.disabled=!savedUndo;
  compare.append(actionStatus,el('div',{class:'magic-part-actionbar'},
   el('button',{class:'ghost',id:'part-open-photocraft',title:'Edit a draft copy in PhotoCraft; save it to use as the candidate','aria-label':`Open ${name} in PhotoCraft`,onclick:()=>openPartInPhotoCraft(selected)},'PhotoCraft'),
   replaceButton,undoButton,
   el('button',{class:'ghost',id:'part-inspect-original',title:selected.file||'','aria-label':`Inspect original ${name}`,onclick:()=>selected.file&&openPath(selected.file)},'Original'),
   el('button',{class:'ghost',id:'part-copy-path',title:'Copy file path to clipboard','aria-label':`Copy asset path for ${name}`,onclick:()=>{if(selected.file&&navigator.clipboard)navigator.clipboard.writeText(selected.file).then(()=>toast('Copied asset path')).catch(()=>{});}},'Copy path')));
 }
 const assetsGrid=(motion.assets||[]).length?el('div',{class:'magic-assets-grid'},motion.assets.map(asset=>magicAssetTile(asset,{active:asset.id===motion.character,onclick:()=>{if(asset.id!==motion.character)openAnimation(asset.id);}}))):el('p',{class:'why'},'No project assets are available yet.');
 const partsSlot=$('magic-parts-slot');
 if(partsSlot)partsSlot.replaceChildren(partsDrawer(compare,assetsGrid));else authoring.layers.append(layers);box.append(section);
 const controls=meta.controls?.joints||[];
 box.append(motionSection('Expression & pose',[el('div',{class:'anim-part-inspector',id:'anim-selected-part-inspector','aria-label':'Selected part controls'}),el('p',{class:'muted'},controls.length?'This rig exposes joint bindings.':'Standard actions move the whole object. Custom tracks add detailed acting to its layers.')],false));
 box.append(motionSection('Quality checks',el('p',{class:'muted'},`${meta.layers.length} layers · ${meta.tracks.length} motion tracks`),false));
 const exportStatus=el('div',{class:'why'});
 const exportButton=el('button',{class:'start',id:'motion-export',title:'Export this action as a transparent video track',onclick:async()=>{
  exportButton.disabled=true;exportStatus.textContent='Rendering transparent track…';
  try{const result=await motionCommand('motion.export_track',{character:motion.character,track:motion.track,amount:motion.amount,duration:motion.sample.duration/motion.speed});
   const file=result.outputs?.[0]?.file;exportStatus.replaceChildren(el('span',{},'Track ready'),file?fileButton({path:file,name:file.split('/').pop()}):null);
  }catch(error){exportStatus.textContent=String(error);}finally{exportButton.disabled=false;}
 }},'Export track');
 box.append(motionSection('Export & integrations',[exportButton,exportStatus,el('button',{id:'motion-mlt-assembly',class:'ghost',title:'Assemble an existing clip/audio timeline with MLT',onclick:()=>nativeAnimationOpen({backend:'mlt'}).catch(error=>toast(String(error)))},'MLT clip/audio assembly'),authoring.pose],false));
 const authoredCount=(meta.tracks||[]).filter(t=>!t.standard).length;
 box.append(el('div',{class:'motion-source-context',id:'motion-source-context'},
  el('strong',{},`${motion.assets?.find(a=>a.id===motion.character)?.name||motion.character} · Subject / shared engine · ${meta.layers.length} layers / ${authoredCount} authored clips`),
  el('span',{class:'muted'},selected?`Selected: ${selected.label||selected.id}`:'Select a part in Layers or Parts & assets')
 ));
 renderPartControlsBar(selected,authoring.part);
 if(typeof theatreSyncClips==='function')theatreSyncClips();
 animationRemember();
}
function renderPartControlsBar(selected,controls){
 const bar=$('anim-selected-part-inspector');if(!bar)return;
 bar.replaceChildren(controls);
 if(!selected)return;
 const character=motion.character,project=state.project,shown=selected.state?.visible!==false;
 const set=async values=>{try{await motionCommand('part.set_state',{expected_project:project,character,part:selected.id,...values});await animationRefresh();}catch(error){toast(String(error));}};
 bar.append(el('p',{class:'muted'},`Layer ${selected.order??'?'} · parent ${selected.parent||'root'} · pivot ${(selected.pivot||[]).join(', ')}`),
 motionButton('motion-bar-visible',`${shown?'Hide':'Show'} ${selected.label||selected.id}`,'eye',()=>set({visible:!shown})),
 motionButton('motion-bar-solo',`${selected.state?.solo?'Unsolo':'Solo'} ${selected.label||selected.id}`,'layers',()=>set({solo:!selected.state?.solo})),
 el('button',{id:'motion-show-all',class:'ghost','data-commands':'part.show_all',onclick:async()=>{try{await motionCommand('part.show_all',{expected_project:project,character});await animationRefresh();}catch(error){toast(String(error));}}},'Show all parts'));
}
// Compare | All parts: one tab at a time, remembered while the view is open.
function partsDrawer(compare,assetsGrid){
 const meta=motion.meta;
 const cards=(meta.layers||[]).map(part=>{
  const shown=part.state?.visible!==false,name=part.label||part.id,picked=motion.selected===part.id;
  const candidate=partCandidate(part.id);
  const select=()=>{motion.selected=part.id;animationControls();animationDraw();updateMagicSelection();};
  const badge=candidate?el('span',{class:'magic-part-dot warn',title:candidate.source==='photocraft'?'A saved PhotoCraft draft awaits review':'A generated candidate awaits review'}):picked?el('span',{class:'magic-part-dot accent',title:'Selected'}):null;
  const visible=motionButton(`magic-part-visible-${part.id}`,`${shown?'Hide':'Show'} ${name}`,'eye',async()=>{await motionCommand('part.set_state',{expected_project:state.project,character:motion.character,part:part.id,visible:!shown});await animationRefresh();});
  visible.setAttribute('aria-pressed',String(shown));
  return el('article',{class:`magic-part-card${picked?' active':''}${shown?'':' hidden-layer'}`,id:`magic-part-card-${part.id}`,title:`${name} (${part.id})`},
   el('button',{class:'magic-part-card-thumb',tabindex:'-1','aria-hidden':'true',onclick:select},el('img',{src:motionAssetUrl(part.file),alt:''})),
   el('button',{class:'magic-part-card-pick','aria-pressed':String(picked),'aria-label':`Select ${name} (${part.id})`,onclick:select},
    el('span',{class:'magic-part-card-name'},name),part.label&&part.label!==part.id?el('span',{class:'magic-part-card-id'},part.id):null,badge),
   el('div',{class:'magic-part-card-actions'},visible,
    el('button',{class:'ghost magic-part-card-open',id:`part-open-photocraft-${part.id}`,title:'Edit a draft copy in PhotoCraft; save it to use as the candidate','aria-label':`Open ${name} in PhotoCraft`,onclick:()=>openPartInPhotoCraft(part)},'PhotoCraft')));
 });
 const panels={parts:cards.length?el('div',{class:'magic-part-cards'},cards):el('p',{class:'why'},'This object has no rig parts.'),compare,assets:assetsGrid};
 const body=el('div',{class:'magic-parts-panel',role:'tabpanel'},panels[motion.partsTab]||compare);
 const tab=(key,label)=>el('button',{class:`magic-parts-tab${motion.partsTab===key?' active':''}`,role:'tab','aria-selected':String(motion.partsTab===key),id:`magic-parts-tab-${key}`,onclick:()=>{motion.partsTab=key;animationControls();}},label);
 return el('div',{class:'magic-parts-drawer'},el('div',{class:'magic-parts-tabs',role:'tablist','aria-label':'Parts and assets'},
  tab('parts',`Parts (${(meta.layers||[]).length})`),tab('compare','Compare'),tab('assets',`Assets (${(motion.assets||[]).length})`)),body);
}
// PhotoCraft edits a draft copy; the live layer only changes through part.replace_image.
async function openPartInPhotoCraft(part){
 try{
  const result=await motionCommand('part.open_photocraft',{asset:motion.character,part:part.id,spawn:true});
  toast(`PhotoCraft: ${(result.draft?.file||result.file).split('/').pop()} · keep it ${result.width}×${result.height} · save to make it the candidate`);
  await refreshPartDrafts();
 }catch(error){toast(String(error));}
}
// A saved PhotoCraft draft that differs from the part's current image is that part's candidate.
function photoDraftCandidate(partId){
 const draft=motion.photoDrafts[partId];
 if(!draft||draft.matches_current)return null;
 return {assetId:motion.character,partId,image:draft.file,result:{width:draft.width,height:draft.height,sha256:draft.sha256},rigSha256:motion.meta?.rig_sha256,partSha256:draft.current_sha256,revision:motion.meta?.image_revision,source:'photocraft'};
}
const partCandidate=partId=>(typeof imageCandidateForPart==='function'?imageCandidateForPart(motion.character,partId):null)||photoDraftCandidate(partId);
const candidateUrl=candidate=>{const url=String(convertFileSrc(candidate.image));return `${url}${url.includes('?')?'&':'?'}v=${candidate.result?.sha256||''}`;};
// Drafts live on disk (.part-image-history/<asset>/drafts); re-read them after loads and on focus.
async function refreshPartDrafts(){
 const character=motion.character,found={};
 for(const part of motion.meta?.layers||[]){
  let result;
  // Rigs without one PNG per part are read-only: they have no drafts to offer.
  try{result=await motionCommand('part.open_photocraft',{asset:character,part:part.id});}catch{break;}
  if(result.draft)found[part.id]={...result.draft,current_sha256:result.sha256};
 }
 if(motion.character!==character||JSON.stringify(found)===JSON.stringify(motion.photoDrafts))return;
 motion.photoDrafts=found;animationControls();
}
window.addEventListener('focus',()=>{if(state.view==='animation'&&motion.mode!=='composite'&&motion.meta)refreshPartDrafts();});
async function animationSample(){
 if(!motion.meta?.tracks?.length)return;
 const request=++motion.request;animationPause();
 $('motion-caption').textContent='Sampling motion…';
 try{const sample=await motionCommand('motion.sample',{character:motion.character,track:motion.track||motion.meta.tracks[0].id,rest:motion.rest,amount:motion.amount});if(request!==motion.request)return;if(sample.layer_ids&&(sample.layer_ids.length!==motion.meta.layers.length||sample.layer_ids.some((id,i)=>id!==motion.meta.layers[i].id)))throw Error('Layer order changed during sampling; refresh the object before playback.');motion.sample=sample;if(typeof timelineRender==='function')timelineRender();motion.time=Math.min(motion.time,sample.duration);$('motion-scrub').max=sample.duration;animationDraw();}catch(e){toast(String(e));}
}
async function animationRefresh(){
 if(state.view!=='animation')return;
 if(motion.mode==='composite'){seqRenderInspector();seqDraw();return;}
 const project=state.project,request=++motion.request;
 animationPause();
 const catalog=await motionCommand('project.list_assets');
 if(state.project!==project||request!==motion.request)return;
 motion.assets=catalog.assets||catalog.project_assets||[];
 const character=motion.assets.some(a=>a.id===motion.character)?motion.character:motion.assets[0]?.id||null;
 if(!character){Object.assign(motion,{character:null,selected:null,track:'',meta:null,sample:null,images:[]});$('motion-inspector')?.replaceChildren(motionModeSwitch());animationEmpty('Bring in your artwork','Choose an object from the Project view.');if(typeof timelineRender==='function')timelineRender();return;}
 const changed=character!==motion.character;
 if(changed)theatreRequireSaved();
 const meta=await motionCommand('motion.tracks',{character});
 const images=await Promise.all((meta.layers||[]).map(l=>new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(new Error(`Could not refresh ${l.label||l.id}`));img.src=motionAssetUrl(l.file);}))); 
 if(state.project!==project||request!==motion.request)return;
 // Publish metadata and clear its previous sample together; controls can render curves immediately.
 Object.assign(motion,{character,meta,images,sample:null});
 if(changed||!meta.layers?.some(l=>l.id===motion.selected))motion.selected=null;
 if(changed||!meta.tracks?.some(t=>t.id===motion.track)){motion.track=meta.tracks?.[0]?.id||'';motion.time=0;motion.rest=true;}
 animationControls();await animationSample();await refreshPartDrafts();
}

function animationSeek(time){
 if(!Number.isFinite(time))return;
 animationPause();motion.time=Math.max(0,Math.min(motion.sample?.duration||0,time));animationDraw();animationRemember();
}
function animationStep(direction){animationSeek(motion.time+direction/(motion.sample?.fps||30));}
function animationPause(){
 if(motion.mode==='composite'){seqPause();return;}
 const wasPlaying=motion.playing;motion.playing=false;motion.playId=(motion.playId||0)+1;
 if(typeof cancelAnimationFrame==='function'&&motion.tick){cancelAnimationFrame(motion.tick);motion.tick=0;}
 if($('motion-play')){$('motion-play').innerHTML=`<svg viewBox="0 0 24 24">${MOTION_ICONS.play}</svg>`;$('motion-play').title='Play motion';$('motion-play').setAttribute('aria-label','Play motion');}
 if(wasPlaying)animationRemember();
 if(wasPlaying&&typeof theatreSeek==='function'&&motion.sample)theatreSeek(motion.time);
}
async function animationPlay(){
 if(motion.mode==='composite'){seqPlay();return;}
 if(motion.playing){animationPause();return;}
 if(motion.rest){motion.rest=false;motion.track=motion.track||motion.meta?.tracks?.[0]?.id;animationControls();await animationSample();}
 if(!motion.sample)return;
 if(motion.time>=motion.sample.duration)motion.time=0;
 const runId=(motion.playId=(motion.playId||0)+1);
 motion.playing=true;$('motion-play').innerHTML=`<svg viewBox="0 0 24 24">${MOTION_ICONS.pause}</svg>`;$('motion-play').title='Pause motion';$('motion-play').setAttribute('aria-label','Pause motion');
 let last=performance.now();function tick(now){if(!motion.playing||motion.playId!==runId)return;motion.time+=(now-last)/1000*motion.speed;last=now;if(motion.time>=motion.sample.duration){if(motion.previewLoop??motion.sample.loop)motion.time%=motion.sample.duration;else{motion.time=motion.sample.duration;animationPause();}}animationDraw();if(motion.playing&&motion.playId===runId)motion.tick=requestAnimationFrame(tick);}motion.tick=requestAnimationFrame(tick);
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
 frame.layers.forEach((l,i)=>{if(!l.opacity)return;ctx.save();ctx.transform(...l.matrix);theatreTransform(ctx,meta.layers[i],l.rotation);ctx.globalAlpha=l.opacity;const crop=meta.layers[i].crop;ctx.drawImage(motion.images[i],crop[0],crop[1]);ctx.restore();});
 drawMotionPath(ctx,scale);
 $('motion-caption').textContent=`${motion.character} · ${motion.rest?'Rest pose':(meta.tracks.find(t=>t.id===motion.track)?.label||motion.track.replaceAll('-',' '))}`;
 $('motion-time').textContent=`${Math.floor(motion.time/60)}:${(motion.time%60).toFixed(3).padStart(6,'0')}`;$('motion-scrub').value=motion.time;
 if($('motion-part-angle')&&document.activeElement!==$('motion-part-angle')){const deg=Math.round((magicTheatre.rotation||0)*10)/10;$('motion-part-angle').value=String(deg);if($('motion-part-angle-out'))$('motion-part-angle-out').textContent=`${deg}°`;}
 if(!motion.playing&&!motion.loading)animationRemember();
 if(!motion.playing||magicTheatre.interactionRevision>magicTheatre.savedRevision)theatreSeek(motion.time);
 if(typeof timelineSeek==='function')timelineSeek(motion.time);
}

function pathControls(){
 const apply=el('button',{id:'motion-path-apply',class:'start',onclick:applyMotionPath},'Apply path');apply.disabled=motion.points.length<2;
 const undo=motionButton('motion-path-undo','Undo the last drawn path','reset',async()=>{if(!motion.pathUndo)return;try{await motionCommand('motion.undo_path',{character:motion.character,undo:motion.pathUndo});motion.pathUndo=null;motion.points=[];await animationLoad();}catch(e){toast(String(e));}});undo.disabled=!motion.pathUndo;
 const roots=(motion.meta?.layers||[]).filter(l=>!l.parent);
 if(!roots.some(l=>l.id===motion.pathLayer))motion.pathLayer=roots[0]?.id||null;
 const target=el('select',{id:'motion-path-layer','aria-label':'Root layer moved by path',onchange:()=>{motion.pathLayer=target.value;}},...roots.map(l=>el('option',{value:l.id},l.label||l.id)));target.value=motion.pathLayer||'';
 const point=el('select',{id:'motion-path-point','aria-label':'Edit path point'},...motion.points.map((p,i)=>el('option',{value:String(i)},`${i+1} · ${p.map(v=>Number(v.toFixed(1))).join(', ')}`)));
 const x=el('input',{id:'motion-path-x',type:'number',step:'any','aria-label':'Path point X'}),y=el('input',{id:'motion-path-y',type:'number',step:'any','aria-label':'Path point Y'});
 const read=()=>{const p=motion.points[Number(point.value)];x.disabled=y.disabled=!p;if(p){x.value=p[0];y.value=p[1];}};
 point.onchange=read;
 for(const [input,index]of [[x,0],[y,1]])input.onchange=()=>{const p=motion.points[Number(point.value)],value=Number(input.value);if(p&&String(input.value).trim()&&Number.isFinite(value)){p[index]=value;animationDraw();}else read();};
 read();
 const section=motionSection('Path & depth',[
  el('label',{},'Path root',target),
  el('label',{},'Edit point',point),el('div',{class:'motion-coordinate-row'},el('label',{},'X',x),el('label',{},'Y',y)),
  el('p',{class:'motion-help',title:'Select Pen above the stage, then draw. Playback moves from the first point at zero to the last point at the chosen duration.'},'Draw a route. Set its timing.'),
  motionRange('motion-path-duration','Duration',0.1,Math.max(600,motion.pathDuration),0.1,motion.pathDuration,'s',v=>{motion.pathDuration=v;animationDraw();}),
  motionRange('motion-size-start','Start size',1,Math.max(1000,motion.startSize),1,motion.startSize,'%',v=>motion.startSize=v),
  motionRange('motion-size-end','End size',1,Math.max(1000,motion.endSize),1,motion.endSize,'%',v=>motion.endSize=v),
  el('div',{class:'motion-path-actions'},motionButton('motion-path-clear','Clear the drawn line','reset',()=>{motion.points=[];$('motion-path-apply').disabled=true;animationDraw();}),undo,apply)
 ],false);section.id='motion-path-section';return section;
}
function setupPathPen(canvas){
 let drawing=false;
 const point=e=>{const v=motion.viewTransform;if(!v)return;const r=canvas.getBoundingClientRect();const p=[(e.clientX-r.left-v.w/2)/v.scale+v.cx,(e.clientY-r.top-v.h/2)/v.scale+v.cy];const last=motion.points.at(-1);if(!last||Math.hypot(p[0]-last[0],p[1]-last[1])*v.scale>4){if(motion.points.length<512)motion.points.push(p);$('motion-path-apply').disabled=motion.points.length<2;animationDraw();}};
 canvas.addEventListener('pointerdown',e=>{if(!motion.pen)return;e.preventDefault();canvas.setPointerCapture(e.pointerId);drawing=true;point(e);});
 canvas.addEventListener('pointermove',e=>{if(drawing)point(e);});
 canvas.addEventListener('pointerup',e=>{if(drawing){point(e);drawing=false;const section=$('motion-path-section');if(section)section.replaceWith(pathControls());}});
 canvas.addEventListener('pointercancel',()=>drawing=false);
}
function drawMotionPath(ctx,scale){
 if(!motion.pathVisible||!motion.points.length)return;ctx.save();ctx.strokeStyle=getComputedStyle(document.documentElement).getPropertyValue('--accent');ctx.fillStyle=ctx.strokeStyle;ctx.lineWidth=1.5/scale;ctx.beginPath();motion.points.forEach((p,i)=>i?ctx.lineTo(...p):ctx.moveTo(...p));ctx.stroke();
 for(const i of [0,motion.points.length-1]){const p=motion.points[i];ctx.beginPath();ctx.arc(...p,4/scale,0,Math.PI*2);ctx.fill();ctx.font=`${12/scale}px system-ui`;ctx.fillText(i===0?'0':`${motion.pathDuration}s`,p[0]+8/scale,p[1]-8/scale);}ctx.restore();
}
async function applyMotionPath(){
 if(!theatreCanLeave())return;
 animationPause();const button=$('motion-path-apply');button.disabled=true;
 try{const result=await motionCommand('motion.path',{character:motion.character,track:motion.track||motion.meta.tracks[0].id,layer:motion.pathLayer,points:motion.points,duration:motion.pathDuration,start_size:motion.startSize,end_size:motion.endSize});
 motion.pathUndo=result.undo;motion.sample=null;motion.track=result.track;motion.rest=false;motion.pen=false;motion.time=0;motion.meta=await motionCommand('motion.tracks',{character:motion.character});animationControls();$('motion-pen').setAttribute('aria-pressed','false');$('motion-canvas').classList.remove('drawing');await animationSample();toast('Path ready · play to review');
 }catch(e){toast(String(e));}finally{if($('motion-path-apply'))$('motion-path-apply').disabled=motion.points.length<2;}
}

window.addEventListener('pagehide',animationRemember);
