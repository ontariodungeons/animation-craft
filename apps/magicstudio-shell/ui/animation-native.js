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
 const backend=el('select',{id:'native-backend','aria-label':'Native animation backend'},el('option',{value:'blender'},'Blender'),el('option',{value:'synfig'},'Synfig'),el('option',{value:'opentoonz'},'OpenToonz'),el('option',{value:'mlt'},'MLT assembly'));backend.value=selection.backend||'synfig';
 const source=field('source','Saved Synfig .sif / .sifz','text'),bundle=field('bundle','Dependency bundle (blank = backend default)','text');
 const optional=[['first_frame','First frame (inclusive)',0,18000],['last_frame','Last frame (inclusive)',0,18000],['fps','Frames per second',0.001,120],['width','Width (pixels)',2,4096],['height','Height (pixels)',2,4096]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const timeline=field('timeline','Saved assembly timeline JSON','text'),blend=field('blend','Saved Blender .blend','text');
 const blenderFields=[['max_frames','Maximum frames',1,18000],['resolution_percent','Resolution percent',10,100]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const tnz=field('tnz','Saved OpenToonz .tnz','text');
 const toonzFields=[['first','First scene frame (one-based)',1,Number.MAX_SAFE_INTEGER],['last','Last scene frame (inclusive)',1,Number.MAX_SAFE_INTEGER],['step','Frame step (held at scene FPS)',1,Number.MAX_SAFE_INTEGER],['shrink','Shrink divisor',1,16]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const threads={key:'nthreads',...field('nthreads','Threads (positive integer, single, half or all)','text')};toonzFields.push(threads);
 const resources=[['memory_mb','Memory limit (MiB)',512,8192],['timeout_seconds','Timeout (seconds)',10,3600]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const status=el('p',{id:'native-status',role:'status'},'Discovering native tools…'),capability=el('p',{class:'muted',id:'native-synfig-status'}),receipt=el('pre',{id:'native-result',class:'animation-readback'}),command=el('code',{id:'native-command',class:'animation-readback'});
 const video=el('video',{controls:'',id:'native-preview','aria-label':'Native rendered preview'});session.video=video;
 const start=el('button',{id:'native-start',class:'start','data-commands':'synfig.start'},'Render Synfig'),cancel=el('button',{id:'native-cancel',class:'ghost'},'Cancel render');
 const jobs=el('select',{id:'native-jobs','aria-label':'Saved native render jobs'}),saved=el('button',{id:'native-saved-source',class:'ghost'},'Use frozen source');saved.disabled=true;
 let activeJob=null,lastReceipt=null,revision=0,rendering=false,selectedBackend=backend.value,requiredBundleFor=null;
 const capable={blender:true,synfig:false,opentoonz:false,mlt:false},names={blender:'Blender',synfig:'Synfig',opentoonz:'OpenToonz',mlt:'MLT assembly'};
 let sources=[],linked=null,unsaved=false,mutating=false;
 const suggestedId=(selection.sourceId||[backend.value,selection.character,selection.track||selection.source?.split('/').pop()?.replace(/\.[^.]+$/,'')].filter(Boolean).join('-')).replace(/[^A-Za-z0-9_-]+/g,'-').slice(0,64)||'native-source';
 const links=el('select',{id:'native-source-links','aria-label':'Registered editable animation source'}),linkId=field('link-id','Source ID','text'),character=field('character','Character ID (optional pair)','text'),track=field('track','Authored action ID (optional pair)','text');
 const linkState=el('p',{id:'native-link-status',role:'status'}),linkReadback=el('pre',{id:'native-link-result',class:'animation-readback'}),linkCommand=el('code',{id:'native-link-command',class:'animation-readback'});
 const register=el('button',{id:'native-source-register',class:'ghost','data-commands':'animation.source.set'},'Register / update source'),accept=el('button',{id:'native-source-accept',class:'ghost','data-commands':'animation.source.accept'},'Accept completed result'),saveProject=el('button',{id:'native-source-save',class:'ghost','data-commands':'project.save'},'Save project'),reload=el('button',{id:'native-source-reload',class:'ghost','data-commands':'animation.sources'},'Reload sources');
 const entry=()=>backend.value==='blender'?blend:backend.value==='mlt'?timeline:backend.value==='opentoonz'?tnz:source;
 const draft=()=>({id:linkId.input.value.trim(),backend:backend.value,source:entry().input.value.trim(),bundle:['synfig','opentoonz'].includes(backend.value)?bundle.input.value.trim()||null:null,character:character.input.value.trim()||null,track:track.input.value.trim()||null});
 const matches=()=>!!linked&&Object.entries(draft()).every(([key,value])=>value===(linked[key]??null));
 const canAccept=()=>matches()&&lastReceipt?.state==='completed'&&lastReceipt.partial===false&&lastReceipt.project===project&&lastReceipt.backend===linked.backend&&lastReceipt.source_id===linked.id&&lastReceipt.source_revision===linked.revision;
 const sourceIdentity=()=>{if(!matches())throw Error('Register the source and action changes before rendering.');return {expected_project:project,id:linked.id,expected_revision:linked.revision};};
 const placement=blenderPlacementControls({project,getJob:()=>lastReceipt,current,jobParameter:'animation_job'});
 const fields=[backend,links,linkId.input,character.input,track.input,blend.input,...blenderFields.map(f=>f.input),source.input,tnz.input,timeline.input,bundle.input,...optional.map(f=>f.input),...toonzFields.map(f=>f.input),...resources.map(f=>f.input),jobs];
 const busy=value=>{rendering=value;start.setAttribute('data-commands','animation.source.render');placement.setBusy(value);start.disabled=value||mutating||!capable[backend.value]||!matches();start.textContent=`Render ${names[backend.value]}`;start.title=!matches()?'Register source changes first':backend.value==='blender'?'Render registered .blend; worker reports runtime errors':capable[backend.value]?'Render registered native source':'Native runtime unavailable; inspect discovery guidance';cancel.disabled=!value;for(const f of fields)f.disabled=value||mutating;saved.disabled=value||mutating||!lastReceipt?.source_bundle?.path;register.disabled=reload.disabled=value||mutating;accept.disabled=value||mutating||!canAccept();accept.title=canAccept()?'Accept this full result for the current source revision':'Requires a matching completed full result; previews cannot be accepted';saveProject.disabled=value||mutating||!unsaved;};
 const report=(text,error=false)=>{if(current()){status.textContent=text;status.className=error?'image-error':'muted';}};
 function request(){
  if(!capable[backend.value])throw Error(`${names[backend.value]} is not render capable; inspect discovery.`);
  const selectedEntry=entry(),key=backend.value==='blender'?'blend':backend.value==='mlt'?'timeline':backend.value==='opentoonz'?'scene':'source';
  if(!selectedEntry.input.value.trim())throw Error(`Choose a saved ${names[backend.value]} source.`);
  if(['synfig','opentoonz'].includes(backend.value)&&selectedEntry.input.value.trim()===requiredBundleFor&&!bundle.input.value.trim())throw Error('Choose the exact frozen dependency bundle; this historical receipt has no root.');
  const p={[key]:selectedEntry.input.value.trim()};if(['synfig','opentoonz'].includes(backend.value)&&bundle.input.value.trim())p.bundle=bundle.input.value.trim();
  for(const f of [...(backend.value==='blender'?blenderFields:backend.value==='mlt'?[]:backend.value==='opentoonz'?toonzFields:optional),...resources]){
   const raw=String(f.input.value).trim();if(!raw)continue;
   if(f.key==='nthreads'&&['single','half','all'].includes(raw)){p.nthreads=raw;continue;}
   const v=Number(raw),min=f.min??1,max=f.max??Number.MAX_SAFE_INTEGER;
   if(!Number.isFinite(v)||v<min||v>max||(f.key!=='fps'&&!Number.isSafeInteger(v)))throw Error(`Invalid ${f.key}: ${min}–${max}`);p[f.key]=v;
  }
  const first=p.first??p.first_frame,last=p.last??p.last_frame;
  if(first!==undefined&&last!==undefined&&last<first)throw Error('Last frame must follow first frame.');
  return p;
 }
 const quote=v=>`'${String(v).replaceAll("'","'\\''")}'`;
 const echo=(id,params)=>`mc exec ${id} ${Object.entries({project,...params}).map(([key,value])=>quote(`${key}=${typeof value==='object'?JSON.stringify(value):value}`)).join(' ')}`;
 function renderRequest(){const p=request();for(const key of ['source','scene','timeline','blend','bundle'])delete p[key];return {...sourceIdentity(),options:p};}
 const cli=()=>{try{command.textContent=echo('animation.source.render',renderRequest());}catch{command.textContent='';}busy(rendering);};
 for(const f of fields)f.addEventListener('input',cli);
 function showRow(row){
  linked=row;links.value=row?.id||'';linkId.input.value=row?.id||'';character.input.value=row?.character||'';track.input.value=row?.track||'';
  if(row){entry().input.value=row.source;bundle.input.value=row.bundle||'';linkReadback.textContent=JSON.stringify(row,null,2);}
  else linkReadback.textContent='';
  linkState.textContent=unsaved?'Source links changed in memory · Save project explicitly.':'Source links read from project. Native files stay native; film placement is separate.';cli();
 }
 function sourceOptions(){links.replaceChildren(el('option',{value:''},'Register a new editable source…'),...sources.map(row=>el('option',{value:row.id},`${row.id} · ${names[row.backend]} · ${row.track||row.source.split('/').pop()} · r${row.revision}`)));}
 async function loadSources(){
  const token=revision,selectedId=linked?.id;const value=await motionCommand('animation.sources',{expected_project:project});if(!current()||revision!==token)return;
  if(value.project!==project||!Array.isArray(value.sources))throw Error('Source readback belongs to another project or is incomplete.');
  sources=value.sources;unsaved=value.unsaved===true;sourceOptions();
  const row=sources.find(row=>row.id===selectedId);if(row)showRow(row);else{
   linked=null;links.value='';
   if(selection.source&&linkId.input.value===suggestedId){let candidate=suggestedId,index=2;while(sources.some(row=>row.id===candidate))candidate=`${suggestedId}-${index++}`;linkId.input.value=candidate;}
   linkState.textContent=selection.source?'Registration draft from saved scene · Register explicitly. Existing sources remain unchanged.':unsaved?'Source links changed in memory · Save project explicitly.':'Select or register an editable source.';cli();
  }
  return row||null;
 }
 reload.onclick=async()=>{if(rendering||mutating)return;try{const row=await loadSources();if(row)await restoreAccepted(row);}catch(error){if(current())linkState.textContent=String(error);}};
 links.onchange=async()=>{
  if(rendering||mutating)return;const row=sources.find(row=>row.id===links.value);backend.value=row?.backend||backend.value;await backend.onchange();if(current()){showRow(row||null);if(row)await restoreAccepted(row);}
 };
 async function mutateSource(id,params){
  if(rendering||mutating)return;const token=revision;mutating=true;busy(rendering);linkCommand.textContent=echo(id,params);
  try{const result=await motionCommand(id,params);session.lastSourceMutation={command:id,result};if(!current()||revision!==token)return;
   if(result.ok!==true||!result.source||!Number.isSafeInteger(result.source.revision))throw Error('Source mutation returned incomplete readback.');
   unsaved=result.unsaved===true;sources=sources.filter(row=>row.id!==result.source.id).concat(result.source);sourceOptions();showRow(result.source);linkReadback.textContent=JSON.stringify(result,null,2);
  }catch(error){if(current()&&revision===token)linkState.textContent=String(error);}
  finally{mutating=false;if(current())busy(rendering);}
 }
 register.onclick=()=>{try{const p=draft();if(!p.id||!p.source)throw Error('Enter a source ID and existing native file.');if(!!p.character!==!!p.track)throw Error('Character and authored action must be supplied together.');for(const key of ['bundle','character','track'])if(p[key]===null)delete p[key];const previous=sources.find(row=>row.id===p.id);if(previous&&previous.id!==linked?.id)throw Error('Select the existing source before updating it.');return mutateSource('animation.source.set',{expected_project:project,...p,expected_revision:linked?.id===p.id?linked.revision:0});}catch(error){linkState.textContent=String(error);}};
 accept.onclick=()=>{try{if(!canAccept())throw Error('Only a matching completed full result can be accepted; previews are excluded.');return mutateSource('animation.source.accept',{...sourceIdentity(),job:lastReceipt.job});}catch(error){linkState.textContent=String(error);}};
 saveProject.onclick=async()=>{
  if(rendering||mutating)return;mutating=true;busy(rendering);const params={expected_project:project};linkCommand.textContent=echo('project.save',params);
  try{const result=await motionCommand('project.save',params);session.lastSourceMutation={command:'project.save',result};if(!current())return;if(result.ok!==true)throw Error('Project Save returned no confirmation.');unsaved=false;linkReadback.textContent=JSON.stringify(result,null,2);linkState.textContent='Project saved · source links and accepted results retained. Native files remain separate.';}
  catch(error){if(current())linkState.textContent=String(error);}finally{mutating=false;if(current())busy(rendering);}
 };
 async function poll(){
  if(!current()||!activeJob)return;
  const job=activeJob,requestRevision=revision,selected=()=>current()&&activeJob===job&&revision===requestRevision;
  try{const value=await motionCommand('animation.status',{job});if(!selected())return;lastReceipt=value;placement.refresh();receipt.textContent=JSON.stringify(value,null,2);
   if(!['completed','cancelled','failed'].includes(value.state)){busy(true);cancel.disabled=value.state==='cancelling';report(value.state==='cancelling'?'Cancelling…':value.progress?.phase||'Rendering…');session.timer=setTimeout(poll,1000);return;}
   busy(false);report(`${value.state}${value.partial?' · partial clip':''}${value.error?` · ${value.error}`:''}`,value.state==='failed');
   if(value.state==='completed'&&value.clip)await blenderLoadPreview(video,value.clip,selected);
  }catch(error){if(selected()){const active=!['completed','cancelled','failed'].includes(lastReceipt?.state);busy(active);cancel.disabled=!active||lastReceipt?.state==='cancelling';report(`${error}${active?' · Retrying status…':''}`,true);if(active)session.timer=setTimeout(poll,1000);}}
 }
 async function list(){const choice=backend.value,value=await motionCommand('animation.list');if(!current()||choice!==backend.value)return;const native=(value.jobs||[]).filter(j=>j.backend===choice).slice(0,100);jobs.replaceChildren(el('option',{value:''},`Choose a saved ${names[backend.value]} job…`),...native.map(j=>el('option',{value:j.job},`${j.state||'saved'} · ${j.job}`)));}
 async function restoreAccepted(row){
  clearTimeout(session.timer);const token=++revision;activeJob=null;lastReceipt=null;cancel.disabled=true;video.pause();video.removeAttribute('src');video.load();
  if(typeof blenderMediaUrl!=='undefined'&&blenderMediaUrl){URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;}placement.refresh();busy(false);
  const accepted=row.accepted;if(!accepted?.job){linkState.textContent=unsaved?'Source links changed in memory · Save project explicitly.':'Editable source selected. No accepted result is attached.';return;}
  const selected=()=>current()&&revision===token&&linked?.id===row.id&&links.value===row.id;
  linkState.textContent='Checking saved accepted-result receipt…';
  try{
   const receiptValue=await motionCommand('animation.status',{job:accepted.job});if(!selected())return;
   const valid=receiptValue.job===accepted.job&&receiptValue.project===project&&receiptValue.backend===row.backend&&receiptValue.source_id===row.id&&receiptValue.source_revision===row.revision-1&&receiptValue.state==='completed'&&receiptValue.partial===false&&receiptValue.clip===accepted.clip&&receiptValue.clip_sha256===accepted.clip_sha256;
   if(!valid){linkState.textContent='Accepted result unavailable: its actual terminal receipt is missing, stale or mismatched. The editable source remains available.';linkReadback.textContent=JSON.stringify({source:row,receipt:receiptValue},null,2);return;}
   lastReceipt=receiptValue;receipt.textContent=JSON.stringify(receiptValue,null,2);placement.refresh();
   const savedJob=Array.from(jobs.options).find(option=>option.value===accepted.job);if(savedJob){savedJob.textContent=`completed · ${accepted.job} · Accepted result`;jobs.value=accepted.job;}
   try{await blenderLoadPreview(video,accepted.clip,selected);if(!selected())return;linkState.textContent='Accepted full result restored from its actual terminal receipt. The editable native source remains separate.';}
   catch(error){if(selected())linkState.textContent=`Accepted result verified; video preview unavailable: ${error}. The editable source remains available.`;}
   linkReadback.textContent=JSON.stringify({source:row,accepted:accepted,receipt:receiptValue},null,2);busy(false);
  }catch(error){if(selected()){lastReceipt=null;placement.refresh();linkState.textContent=`Accepted result unavailable: terminal receipt could not be read (${error}). The editable source remains available.`;linkReadback.textContent=JSON.stringify({source:row,accepted:accepted,receipt_error:String(error)},null,2);busy(false);}}
 }
 jobs.onchange=async()=>{clearTimeout(session.timer);revision++;activeJob=jobs.value||null;lastReceipt=null;placement.refresh();busy(false);if(activeJob)await poll();};
 start.onclick=async()=>{if(rendering||mutating)return;let params;try{params=renderRequest();}catch(error){report(String(error),true);return;}revision++;clearTimeout(session.timer);busy(true);cancel.disabled=true;cli();report(`Submitting ${names[backend.value]}…`);try{const result=await motionCommand('animation.source.render',params);if(!current())return;if(!result.job)throw Error('Start returned no job identity');activeJob=result.job;lastReceipt=null;placement.refresh();await poll();}catch(error){if(current()){busy(false);report(String(error),true);}}};
 cancel.onclick=async()=>{cancel.disabled=true;try{await motionCommand('animation.cancel',{job:activeJob});if(current()){clearTimeout(session.timer);await poll();}}catch(error){report(String(error),true);}};
 saved.onclick=()=>{const path=lastReceipt?.source_bundle?.path;if(!path)return;entry().input.value=path;bundle.input.value=lastReceipt.source_bundle.bundle||'';requiredBundleFor=lastReceipt.source_bundle.bundle?null:path;for(const f of backend.value==='blender'?blenderFields:backend.value==='mlt'?[]:backend.value==='opentoonz'?toonzFields:optional)f.input.value='';cli();report(backend.value==='mlt'?'Frozen assembly timeline selected.':lastReceipt.source_bundle.bundle?'Frozen source and exact bundle selected; choose the desired range.':'Historical receipt has no bundle root; choose its exact dependency bundle before rendering.');};
 const toonz=el('p',{id:'native-opentoonz-status',role:'status'}),toonzPlan=el('pre',{class:'animation-readback'});
 const discoverToonz=async()=>{try{const value=await motionCommand('opentoonz.discover',tnz.input.value.trim()?{scene:tnz.input.value.trim()}:{});if(!current())return;capable.opentoonz=value.render_capable===true;toonz.textContent=[value.available?'OpenToonz executable found':value.reason||'OpenToonz executable unavailable',value.render_reason,'Requires tcomposer and stuff resources (OPENTOONZ_TCOMPOSER / OPENTOONZ_STUFF). Capability discovery is not native render verification.'].filter(Boolean).join(' · ');toonzPlan.textContent=JSON.stringify(value,null,2);busy(rendering);cli();}catch(error){if(current()){capable.opentoonz=false;toonz.textContent=String(error);busy(rendering);}}};
 const discover=el('button',{class:'ghost',id:'native-opentoonz-discover',onclick:discoverToonz},'Discover OpenToonz');fields.push(discover);
 const blenderGroup=el('div',{id:'native-blender-fields',class:'animation-authoring-fields native-backend-fields'},blend.node,...blenderFields.map(f=>f.node),el('p',{class:'muted'},'Renders an existing editable .blend. Worker errors report unavailable runtime; this does not convert native edits into rig keys.'));
 const synfigGroup=el('div',{id:'native-synfig-fields',class:'animation-authoring-fields native-backend-fields'},capability,source.node,...optional.map(f=>f.node));
 const toonzGroup=el('div',{id:'native-opentoonz-fields',class:'animation-authoring-fields native-backend-fields'},tnz.node,...toonzFields.map(f=>f.node),discover,toonz,el('details',{},el('summary',{},'OpenToonz discovery plan'),toonzPlan));toonzGroup.hidden=true;
 const mltStatus=el('p',{id:'native-mlt-status',class:'muted'}),audioNote=el('p',{id:'native-mlt-audio',class:'muted'},'MLT exports the audio selected in the saved timeline. HyperFrames picture placement stays muted; it does not route that audio into the film soundtrack.');
 const mltGroup=el('div',{id:'native-mlt-fields',class:'animation-authoring-fields native-backend-fields'},mltStatus,timeline.node,el('p',{class:'muted'},'Assembly concatenates local clips at matching FPS, with their audio or an explicit replacement bed from the JSON. The job saves an editable MLT project and opaque MP4.'),audioNote);
 const setGroups=()=>{blenderGroup.hidden=backend.value!=='blender';synfigGroup.hidden=backend.value!=='synfig';toonzGroup.hidden=backend.value!=='opentoonz';mltGroup.hidden=backend.value!=='mlt';bundle.node.hidden=!['synfig','opentoonz'].includes(backend.value);};bundle.node.setAttribute('class','native-backend-fields');setGroups();
 const discoverMlt=async()=>{try{const value=await motionCommand('mlt.discover');if(!current())return;capable.mlt=value.configured===true;mltStatus.textContent=[capable.mlt?'MLT runtime configured':'MLT runtime unavailable',value.render_reason,...(value.unsupported||[])].filter(Boolean).join(' · ');busy(rendering);cli();}catch(error){if(current())mltStatus.textContent=String(error);}};
 backend.onchange=async()=>{if(rendering||mutating){backend.value=selectedBackend;return;}selectedBackend=backend.value;revision++;clearTimeout(session.timer);activeJob=null;lastReceipt=null;video.pause();video.removeAttribute('src');video.load();placement.refresh();setGroups();busy(false);cli();report('Choose a saved native source.');try{await list();}catch(error){report(String(error),true);}};
 const close=el('button',{id:'native-close',class:'ghost',onclick:nativeAnimationDispose},'Close');
 const dialogNode=el('dialog',{id:'native-render-dialog',class:'blender-render-dialog','aria-label':'Native animation source tools'},el('div',{class:'blender-render-header'},el('strong',{},'Native source rendering'),close),el('div',{class:'native-render-body'},el('label',{},'Backend',backend),el('label',{},'Editable source',links),reload,el('details',{open:true},el('summary',{},'Register source / action'),linkId.node,character.node,track.node,blenderGroup,synfigGroup,toonzGroup,mltGroup,bundle.node,register),linkState,accept,saveProject,el('details',{},el('summary',{},'Source / accepted result / Save readback'),linkCommand,linkReadback),el('p',{class:'muted'},'Blank options use saved source settings. OpenToonz scene frames are one-based; stepped frames are held at scene FPS. Rendering supplements authoring.'),el('details',{},el('summary',{},'Resource limits'),...resources.map(f=>f.node)),start,cancel,status,jobs,saved,video,placement.node,el('details',{},el('summary',{},'CLI and job receipt'),command,receipt)));

 session.dialog=dialogNode;dialogNode.addEventListener('close',()=>{if(nativeAnimationSession===session)nativeAnimationDispose()});document.body.append(dialogNode);dialogNode.showModal();busy(false);
 const discoverSynfig=async()=>{try{const value=await motionCommand('synfig.discover');if(!current())return;capable.synfig=value.available===true;capability.textContent=[capable.synfig?'Synfig available':'Synfig unavailable',value.install_dependency,...(value.limitations||[])].filter(Boolean).join(' · ');busy(rendering);}catch(error){if(current())capability.textContent=String(error);}};
 if(selection.source){entry().input.value=selection.source;character.input.value=selection.character||'';track.input.value=selection.track||'';linkId.input.value=suggestedId;cli();}
 await Promise.all([discoverSynfig(),discoverToonz(),discoverMlt(),placement.load(),loadSources().catch(error=>{if(current())linkState.textContent=String(error);}),list().catch(error=>report(String(error),true))]);
 if(current())report(capable[backend.value]?'Select or register an editable source to render.':'Native runtime unavailable; inspect discovery guidance.',!capable[backend.value]);
}
