// Native source rendering supplements the mounted Subject/Composite authoring editor.
let nativeAnimationSession=null;
function nativeAnimationDispose(){
 const session=nativeAnimationSession;nativeAnimationSession=null;if(!session)return;
 clearTimeout(session.timer);if(typeof blenderMediaFetch!=='undefined')blenderMediaFetch?.abort();if(typeof blenderMediaUrl!=='undefined'&&blenderMediaUrl){URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;}session.video.pause();session.video.removeAttribute('src');session.video.load();session.dialog.close();session.dialog.remove();
}
async function nativeAnimationOpen(selection={}){
 theatreRequireSaved();blenderDispose();
 const project=state.project,session={timer:null},current=()=>nativeAnimationSession===session&&state.project===project&&state.view==='animation';nativeAnimationSession=session;
 const field=(key,label,type='number')=>{const input=el('input',{id:`native-${key}`,type,step:'any','aria-label':label});return {input,node:el('label',{},label,input)}};
 const backend=el('select',{id:'native-backend','aria-label':'Native animation backend'},el('option',{value:'synfig'},'Synfig'),el('option',{value:'opentoonz'},'OpenToonz'),el('option',{value:'mlt'},'MLT assembly'));backend.value=selection.backend||'synfig';
 const source=field('source','Saved Synfig .sif / .sifz','text'),bundle=field('bundle','Dependency bundle (blank = backend default)','text');
 const optional=[['first_frame','First frame (inclusive)',0,18000],['last_frame','Last frame (inclusive)',0,18000],['fps','Frames per second',0.001,120],['width','Width (pixels)',2,4096],['height','Height (pixels)',2,4096]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const timeline=field('timeline','Saved assembly timeline JSON','text');
 const tnz=field('tnz','Saved OpenToonz .tnz','text');
 const toonzFields=[['first','First scene frame (one-based)',1,Number.MAX_SAFE_INTEGER],['last','Last scene frame (inclusive)',1,Number.MAX_SAFE_INTEGER],['step','Frame step (held at scene FPS)',1,Number.MAX_SAFE_INTEGER],['shrink','Shrink divisor',1,16]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const threads={key:'nthreads',...field('nthreads','Threads (positive integer, single, half or all)','text')};toonzFields.push(threads);
 const resources=[['memory_mb','Memory limit (MiB)',512,8192],['timeout_seconds','Timeout (seconds)',10,3600]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const status=el('p',{id:'native-status',role:'status'},'Discovering native tools…'),capability=el('p',{class:'muted',id:'native-synfig-status'}),receipt=el('pre',{id:'native-result',class:'animation-readback'}),command=el('code',{id:'native-command',class:'animation-readback'});
 const video=el('video',{controls:'',id:'native-preview','aria-label':'Native rendered preview'});session.video=video;
 const start=el('button',{id:'native-start',class:'start','data-commands':'synfig.start'},'Render Synfig'),cancel=el('button',{id:'native-cancel',class:'ghost'},'Cancel render');
 const jobs=el('select',{id:'native-jobs','aria-label':'Saved native render jobs'}),saved=el('button',{id:'native-saved-source',class:'ghost'},'Use frozen source');saved.disabled=true;
 let activeJob=null,lastReceipt=null,revision=0,rendering=false,selectedBackend=backend.value,requiredBundleFor=null;
 const capable={synfig:false,opentoonz:false,mlt:false},names={synfig:'Synfig',opentoonz:'OpenToonz',mlt:'MLT assembly'};
 const placement=blenderPlacementControls({project,getJob:()=>lastReceipt,current,jobParameter:'animation_job'});
 const fields=[backend,source.input,tnz.input,timeline.input,bundle.input,...optional.map(f=>f.input),...toonzFields.map(f=>f.input),...resources.map(f=>f.input),jobs];
 const busy=value=>{rendering=value;start.setAttribute('data-commands',`${backend.value}.start`);placement.setBusy(value);start.disabled=value||!capable[backend.value];start.textContent=`Render ${names[backend.value]}`;start.title=capable[backend.value]?'Render saved native source':'Native runtime unavailable; inspect discovery guidance';cancel.disabled=!value;for(const f of fields)f.disabled=value;saved.disabled=value||!lastReceipt?.source_bundle?.path;};
 const report=(text,error=false)=>{if(current()){status.textContent=text;status.className=error?'image-error':'muted';}};
 function request(){
  if(!capable[backend.value])throw Error(`${names[backend.value]} is not render capable; inspect discovery.`);
  const entry=backend.value==='mlt'?timeline:backend.value==='opentoonz'?tnz:source,key=backend.value==='mlt'?'timeline':backend.value==='opentoonz'?'scene':'source';
  if(!entry.input.value.trim())throw Error(`Choose a saved ${names[backend.value]} source.`);
  if(backend.value!=='mlt'&&entry.input.value.trim()===requiredBundleFor&&!bundle.input.value.trim())throw Error('Choose the exact frozen dependency bundle; this historical receipt has no root.');
  const p={[key]:entry.input.value.trim()};if(backend.value!=='mlt'&&bundle.input.value.trim())p.bundle=bundle.input.value.trim();
  for(const f of [...(backend.value==='mlt'?[]:backend.value==='opentoonz'?toonzFields:optional),...resources]){
   const raw=String(f.input.value).trim();if(!raw)continue;
   if(f.key==='nthreads'&&['single','half','all'].includes(raw)){p.nthreads=raw;continue;}
   const v=Number(raw),min=f.min??1,max=f.max??Number.MAX_SAFE_INTEGER;
   if(!Number.isFinite(v)||v<min||v>max||(f.key!=='fps'&&!Number.isSafeInteger(v)))throw Error(`Invalid ${f.key}: ${min}–${max}`);p[f.key]=v;
  }
  const first=p.first??p.first_frame,last=p.last??p.last_frame;
  if(first!==undefined&&last!==undefined&&last<first)throw Error('Last frame must follow first frame.');
  return p;
 }
 const cli=()=>{try{const q=v=>`'${String(v).replaceAll("'","'\\''")}'`;command.textContent=`mc exec ${backend.value}.start ${q(`project=${project}`)} ${Object.entries(request()).map(([k,v])=>q(`${k}=${v}`)).join(' ')}`;}catch{command.textContent='';}};
 for(const f of fields)f.addEventListener('input',cli);
 async function poll(){
  if(!current()||!activeJob)return;
  const job=activeJob,requestRevision=revision,selected=()=>current()&&activeJob===job&&revision===requestRevision;
  try{const value=await motionCommand('animation.status',{job});if(!selected())return;lastReceipt=value;placement.refresh();receipt.textContent=JSON.stringify(value,null,2);
   if(value.state==='running'||value.state==='cancelling'){busy(true);cancel.disabled=value.state==='cancelling';report(value.state==='cancelling'?'Cancelling…':value.progress?.phase||'Rendering…');session.timer=setTimeout(poll,1000);return;}
   busy(false);report(`${value.state}${value.partial?' · partial clip':''}${value.error?` · ${value.error}`:''}`,value.state==='failed');
   if(value.state==='completed'&&value.clip)await blenderLoadPreview(video,value.clip,selected);
  }catch(error){if(selected()){const active=['running','cancelling'].includes(lastReceipt?.state);busy(active);report(String(error),true);if(active)session.timer=setTimeout(poll,1000);}}
 }
 async function list(){const choice=backend.value,value=await motionCommand('animation.list');if(!current()||choice!==backend.value)return;const native=(value.jobs||[]).filter(j=>j.backend===choice).slice(0,100);jobs.replaceChildren(el('option',{value:''},`Choose a saved ${names[backend.value]} job…`),...native.map(j=>el('option',{value:j.job},`${j.state||'saved'} · ${j.job}`)));}
 jobs.onchange=async()=>{clearTimeout(session.timer);revision++;activeJob=jobs.value||null;lastReceipt=null;placement.refresh();busy(false);if(activeJob)await poll();};
 start.onclick=async()=>{if(rendering)return;let params;try{params=request();}catch(error){report(String(error),true);return;}revision++;clearTimeout(session.timer);busy(true);cancel.disabled=true;cli();report(`Submitting ${names[backend.value]}…`);try{const result=await motionCommand(`${backend.value}.start`,params);if(!current())return;if(!result.job)throw Error('Start returned no job identity');activeJob=result.job;await poll();}catch(error){if(current()){busy(false);report(String(error),true);}}};
 cancel.onclick=async()=>{cancel.disabled=true;try{await motionCommand('animation.cancel',{job:activeJob});if(current()){clearTimeout(session.timer);await poll();}}catch(error){report(String(error),true);}};
 saved.onclick=()=>{const path=lastReceipt?.source_bundle?.path;if(!path)return;(backend.value==='mlt'?timeline:backend.value==='opentoonz'?tnz:source).input.value=path;bundle.input.value=lastReceipt.source_bundle.bundle||'';requiredBundleFor=lastReceipt.source_bundle.bundle?null:path;for(const f of backend.value==='mlt'?[]:backend.value==='opentoonz'?toonzFields:optional)f.input.value='';cli();report(backend.value==='mlt'?'Frozen assembly timeline selected.':lastReceipt.source_bundle.bundle?'Frozen source and exact bundle selected; choose the desired range.':'Historical receipt has no bundle root; choose its exact dependency bundle before rendering.');};
 const toonz=el('p',{id:'native-opentoonz-status',role:'status'}),toonzPlan=el('pre',{class:'animation-readback'});
 const discoverToonz=async()=>{try{const value=await motionCommand('opentoonz.discover',tnz.input.value.trim()?{scene:tnz.input.value.trim()}:{});if(!current())return;capable.opentoonz=value.render_capable===true;toonz.textContent=[value.available?'OpenToonz executable found':value.reason||'OpenToonz executable unavailable',value.render_reason,'Requires tcomposer and stuff resources (OPENTOONZ_TCOMPOSER / OPENTOONZ_STUFF). Capability discovery is not native render verification.'].filter(Boolean).join(' · ');toonzPlan.textContent=JSON.stringify(value,null,2);busy(rendering);cli();}catch(error){if(current()){capable.opentoonz=false;toonz.textContent=String(error);busy(rendering);}}};
 const discover=el('button',{class:'ghost',id:'native-opentoonz-discover',onclick:discoverToonz},'Discover OpenToonz');fields.push(discover);
 const synfigGroup=el('div',{id:'native-synfig-fields',class:'animation-authoring-fields native-backend-fields'},capability,source.node,...optional.map(f=>f.node));
 const toonzGroup=el('div',{id:'native-opentoonz-fields',class:'animation-authoring-fields native-backend-fields'},tnz.node,...toonzFields.map(f=>f.node),discover,toonz,el('details',{},el('summary',{},'OpenToonz discovery plan'),toonzPlan));toonzGroup.hidden=true;
 const mltStatus=el('p',{id:'native-mlt-status',class:'muted'}),audioNote=el('p',{id:'native-mlt-audio',class:'muted'},'MLT exports the audio selected in the saved timeline. HyperFrames picture placement stays muted; it does not route that audio into the film soundtrack.');
 const mltGroup=el('div',{id:'native-mlt-fields',class:'animation-authoring-fields native-backend-fields'},mltStatus,timeline.node,el('p',{class:'muted'},'Assembly concatenates local clips at matching FPS, with their audio or an explicit replacement bed from the JSON. The job saves an editable MLT project and opaque MP4.'),audioNote);
 const setGroups=()=>{synfigGroup.hidden=backend.value!=='synfig';toonzGroup.hidden=backend.value!=='opentoonz';mltGroup.hidden=backend.value!=='mlt';bundle.node.hidden=backend.value==='mlt';};bundle.node.setAttribute('class','native-backend-fields');setGroups();
 const discoverMlt=async()=>{try{const value=await motionCommand('mlt.discover');if(!current())return;capable.mlt=value.configured===true;mltStatus.textContent=[capable.mlt?'MLT runtime configured':'MLT runtime unavailable',value.render_reason,...(value.unsupported||[])].filter(Boolean).join(' · ');busy(rendering);cli();}catch(error){if(current())mltStatus.textContent=String(error);}};
 backend.onchange=async()=>{if(rendering){backend.value=selectedBackend;return;}selectedBackend=backend.value;revision++;clearTimeout(session.timer);activeJob=null;lastReceipt=null;video.pause();video.removeAttribute('src');video.load();placement.refresh();setGroups();busy(false);cli();report('Choose a saved native source.');try{await list();}catch(error){report(String(error),true);}};
 const close=el('button',{id:'native-close',class:'ghost',onclick:nativeAnimationDispose},'Close');
 const dialogNode=el('dialog',{id:'native-render-dialog',class:'blender-render-dialog','aria-label':'Native animation source tools'},el('div',{class:'blender-render-header'},el('strong',{},'Native source rendering'),close),el('div',{class:'native-render-body'},el('label',{},'Backend',backend),synfigGroup,toonzGroup,mltGroup,bundle.node,el('p',{class:'muted'},'Blank options use saved source settings. OpenToonz scene frames are one-based; stepped frames are held at scene FPS. Rendering supplements authoring.'),el('details',{},el('summary',{},'Resource limits'),...resources.map(f=>f.node)),start,cancel,status,jobs,saved,video,placement.node,el('details',{},el('summary',{},'CLI and job receipt'),command,receipt)));

 session.dialog=dialogNode;dialogNode.addEventListener('close',()=>{if(nativeAnimationSession===session)nativeAnimationDispose()});document.body.append(dialogNode);dialogNode.showModal();busy(false);
 const discoverSynfig=async()=>{try{const value=await motionCommand('synfig.discover');if(!current())return;capable.synfig=value.available===true;capability.textContent=[capable.synfig?'Synfig available':'Synfig unavailable',value.install_dependency,...(value.limitations||[])].filter(Boolean).join(' · ');busy(rendering);}catch(error){if(current())capability.textContent=String(error);}};
 await Promise.all([discoverSynfig(),discoverToonz(),discoverMlt(),placement.load(),list().catch(error=>report(String(error),true))]);
 if(current())report(capable[backend.value]?'Choose a saved source to render.':'Native runtime unavailable; inspect discovery guidance.',!capable[backend.value]);
}
