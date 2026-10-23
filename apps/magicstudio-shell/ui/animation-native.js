// Native source rendering supplements the mounted Subject/Composite authoring editor.
let nativeAnimationSession=null;
function nativeAnimationDispose(){
 const session=nativeAnimationSession;nativeAnimationSession=null;if(!session)return;
 clearTimeout(session.timer);if(typeof blenderMediaFetch!=='undefined')blenderMediaFetch?.abort();if(typeof blenderMediaUrl!=='undefined'&&blenderMediaUrl){URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;}session.video.pause();session.video.removeAttribute('src');session.video.load();session.dialog.close();session.dialog.remove();
}
async function nativeAnimationOpen(){
 theatreRequireSaved();blenderDispose();
 const project=state.project,session={timer:null},current=()=>nativeAnimationSession===session&&state.project===project&&state.view==='animation';nativeAnimationSession=session;
 const field=(key,label,type='number')=>{const input=el('input',{id:`native-${key}`,type,step:'any','aria-label':label});return {input,node:el('label',{},label,input)}};
 const source=field('source','Saved Synfig .sif / .sifz','text'),bundle=field('bundle','Dependency bundle (blank = source folder)','text');
 const optional=[['first_frame','First frame (inclusive)',0,18000],['last_frame','Last frame (inclusive)',0,18000],['fps','Frames per second',0.001,120],['width','Width (pixels)',2,4096],['height','Height (pixels)',2,4096]].map(([key,label,min,max])=>({key,min,max,...field(key,label)}));
 const status=el('p',{id:'native-status',role:'status'},'Discovering native tools…'),capability=el('p',{class:'muted',id:'native-synfig-status'}),receipt=el('pre',{id:'native-result',class:'animation-readback'}),command=el('code',{id:'native-command',class:'animation-readback'});
 const video=el('video',{controls:'',id:'native-preview','aria-label':'Native rendered preview'});session.video=video;
 const start=el('button',{id:'native-start',class:'start','data-commands':'synfig.start'},'Render Synfig'),cancel=el('button',{id:'native-cancel',class:'ghost'},'Cancel render');
 const jobs=el('select',{id:'native-jobs','aria-label':'Saved native render jobs'}),saved=el('button',{id:'native-saved-source',class:'ghost'},'Use frozen source');saved.disabled=true;
 let available=false,activeJob=null,lastReceipt=null,revision=0;
 const fields=[source.input,bundle.input,...optional.map(f=>f.input),jobs];
 const busy=value=>{start.disabled=value||!available;cancel.disabled=!value;for(const f of fields)f.disabled=value;saved.disabled=value||!lastReceipt?.source_bundle?.path;};
 const report=(text,error=false)=>{if(current()){status.textContent=text;status.className=error?'image-error':'muted';}};
 function request(){
  if(!available)throw Error('Synfig is unavailable; inspect discovery.');
  if(!source.input.value.trim())throw Error('Choose a saved Synfig source.');
  const p={source:source.input.value.trim()};if(bundle.input.value.trim())p.bundle=bundle.input.value.trim();
  for(const f of optional){if(!String(f.input.value).trim())continue;const v=Number(f.input.value);if(!Number.isFinite(v)||v<f.min||v>f.max||(f.key!=='fps'&&!Number.isInteger(v)))throw Error(`Invalid ${f.key}: ${f.min}–${f.max}`);p[f.key]=v;}
  if(p.first_frame!==undefined&&p.last_frame!==undefined&&p.last_frame<p.first_frame)throw Error('Last frame must follow first frame.');
  return p;
 }
 const cli=()=>{try{const q=v=>`'${String(v).replaceAll("'","'\\''")}'`;command.textContent=`mc exec synfig.start ${q(`project=${project}`)} ${Object.entries(request()).map(([k,v])=>q(`${k}=${v}`)).join(' ')}`;}catch{command.textContent='';}};
 for(const f of fields)f.addEventListener('input',cli);
 async function poll(){
  if(!current()||!activeJob)return;
  const job=activeJob,requestRevision=revision,selected=()=>current()&&activeJob===job&&revision===requestRevision;
  try{const value=await motionCommand('animation.status',{job});if(!selected())return;lastReceipt=value;receipt.textContent=JSON.stringify(value,null,2);
   if(value.state==='running'||value.state==='cancelling'){busy(true);cancel.disabled=value.state==='cancelling';report(value.state==='cancelling'?'Cancelling…':value.progress?.phase||'Rendering…');session.timer=setTimeout(poll,1000);return;}
   busy(false);report(`${value.state}${value.partial?' · partial clip':''}${value.error?` · ${value.error}`:''}`,value.state==='failed');
   if(value.state==='completed'&&value.clip)await blenderLoadPreview(video,value.clip,selected);
  }catch(error){if(selected()){const active=['running','cancelling'].includes(lastReceipt?.state);busy(active);report(String(error),true);if(active)session.timer=setTimeout(poll,1000);}}
 }
 async function list(){const value=await motionCommand('animation.list');if(!current())return;const native=(value.jobs||[]).filter(j=>j.backend==='synfig').slice(0,100);jobs.replaceChildren(el('option',{value:''},'Choose a saved Synfig job…'),...native.map(j=>el('option',{value:j.job},`${j.state||'saved'} · ${j.job}`)));}
 jobs.onchange=async()=>{clearTimeout(session.timer);revision++;activeJob=jobs.value||null;lastReceipt=null;busy(false);if(activeJob)await poll();};
 start.onclick=async()=>{let params;try{params=request();}catch(error){report(String(error),true);return;}revision++;busy(true);cancel.disabled=true;report('Submitting Synfig…');try{const result=await motionCommand('synfig.start',params);if(!current())return;if(!result.job)throw Error('Start returned no job identity');activeJob=result.job;await poll();}catch(error){if(current()){busy(false);report(String(error),true);}}};
 cancel.onclick=async()=>{cancel.disabled=true;try{await motionCommand('animation.cancel',{job:activeJob});if(current()){clearTimeout(session.timer);await poll();}}catch(error){report(String(error),true);}};
 saved.onclick=()=>{const path=lastReceipt?.source_bundle?.path;if(!path)return;source.input.value=path;bundle.input.value=path.slice(0,path.lastIndexOf('/'));cli();report('Frozen source selected; original source is unchanged.');};
 const tnz=field('tnz','Saved OpenToonz .tnz (optional discovery plan)','text'),toonz=el('p',{id:'native-opentoonz-status',role:'status'}),toonzPlan=el('pre',{class:'animation-readback'});
 const discoverToonz=async()=>{try{const value=await motionCommand('opentoonz.discover',tnz.input.value.trim()?{scene:tnz.input.value.trim()}:{});if(!current())return;toonz.textContent=`${value.available?'OpenToonz renderer found':value.reason||'OpenToonz unavailable'}. Shared OpenToonz start/encoding is not integrated; worker helper is not a UI render command.`;toonzPlan.textContent=JSON.stringify(value,null,2);}catch(error){if(current())toonz.textContent=String(error);}};
 const close=el('button',{id:'native-close',class:'ghost',onclick:nativeAnimationDispose},'Close');
 const dialogNode=el('dialog',{id:'native-render-dialog',class:'blender-render-dialog','aria-label':'Native animation source tools'},el('div',{class:'blender-render-header'},el('strong',{},'Native source rendering'),close),el('div',{class:'native-render-body'},capability,source.node,bundle.node,el('p',{class:'muted'},'Blank range, rate and dimensions use the saved source. Rendering does not replace authoring.'),...optional.map(f=>f.node),start,cancel,status,jobs,saved,video,el('details',{},el('summary',{},'CLI and job receipt'),command,receipt),el('details',{},el('summary',{},'OpenToonz availability'),tnz.node,el('button',{class:'ghost',id:'native-opentoonz-discover',onclick:discoverToonz},'Discover OpenToonz'),toonz,toonzPlan)));
 session.dialog=dialogNode;dialogNode.addEventListener('close',()=>{if(nativeAnimationSession===session)nativeAnimationDispose()});document.body.append(dialogNode);dialogNode.showModal();busy(false);
 try{const value=await motionCommand('synfig.discover');if(!current())return;available=value.available===true;capability.textContent=[available?'Synfig available':'Synfig unavailable',value.install_dependency,...(value.limitations||[])].filter(Boolean).join(' · ');busy(false);report(available?'Choose a saved source to render.':'Synfig unavailable; inspect the dependency above.',!available);await Promise.all([list(),discoverToonz()]);}catch(error){report(String(error),true);}
}
