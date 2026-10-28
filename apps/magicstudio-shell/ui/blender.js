// Blender owns animation data; this surface only chooses jobs and plays rendered media.
let blenderViewRevision=0,blenderDialog=null;
let blenderMediaUrl=null,blenderMediaFetch=null,blenderPollTimer=null;
function blenderDispose(){
 if(typeof nativeAnimationDispose==='function')nativeAnimationDispose();
 clearTimeout(blenderPollTimer);blenderPollTimer=null;
 ++blenderViewRevision;blenderMediaFetch?.abort();blenderMediaFetch=null;
 const root=blenderDialog;blenderDialog=null;
 const video=root?.querySelector('video');
 if(video){video.pause();video.removeAttribute('src');video.load();}
 if(blenderMediaUrl)URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;
 if(root&&root.tagName==='DIALOG'){root.close();root.remove();}
}
async function blenderLoadPreview(video,path,current){
 if(blenderMediaUrl)URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;
 blenderMediaFetch?.abort();blenderMediaFetch=new AbortController();
 const response=await fetch(convertFileSrc(path),{signal:blenderMediaFetch.signal});
 if(!response.ok)throw new Error('Cannot read the rendered preview');
 const reader=response.body.getReader(),chunks=[];let size=0;
 while(true){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;
  if(size>64*1024*1024){await reader.cancel();throw new Error('Preview exceeds 64 MB; render a shorter or smaller preview');}chunks.push(value);}
 if(!current())return;
 blenderMediaUrl=URL.createObjectURL(new Blob(chunks,{type:String(path).toLowerCase().endsWith('.mp4')?'video/mp4':'video/webm'}));
 video.src=blenderMediaUrl;video.load();
}
async function blenderAnimationOpen(selection={}){
 theatreRequireSaved();
 blenderDispose();
 const choice={character:selection.character??motion.character,track:selection.track??motion.track};
 const revision=++blenderViewRevision,project=state.project;
 const view=$('view-animation');
 view.replaceChildren();
 blenderDialog=view;
 const current=()=>revision===blenderViewRevision&&state.project===project&&state.view==='animation'&&motion.mode==='blender'&&blenderDialog===view;
 const controls=el('aside',{class:'motion-inspector blender-controls',id:'blender-controls','aria-label':'Blender animation controls'});
 const stage=el('div',{class:'motion-stage'});
 const status=el('span',{class:'muted',id:'blender-status',role:'status'},'Loading artwork…');
 const main=el('div',{class:'motion-main'},el('div',{class:'motion-stagebar',id:'motion-stagebar'},motionModeSwitch(),status),stage);
 view.append(controls,main);
 const video=el('video',{id:'blender-preview',controls:true,loop:true,'aria-label':'Rendered Blender animation'});
 stage.append(video);
 let latest=null,job=null,jobState=null,selectionRevision=0;
 const report=(message,detail=message)=>{if(current()){status.textContent=message;status.title=detail;}};
 try{
  const result=await motionCommand('project.list_assets');if(!current())return;
  const assets=result.assets||result.project_assets||[];
  const candidates=assets.filter(a=>a.kind==='cutout_rig');
  if(!candidates.some(a=>a.id===choice.character))choice.character=candidates[0]?.id;
  if(!choice.character){report('Choose a rigged character in Project to begin.');return;}
  motion.character=choice.character;
  const picker=el('select',{'aria-label':'Animated object',id:'blender-object'},...candidates.map(a=>el('option',{value:a.id},a.name||a.id)));
  picker.value=choice.character;
  picker.onchange=()=>{motion.character=picker.value;blenderAnimationOpen({character:picker.value,track:choice.track});};
  controls.append(picker);
  const meta=await motionCommand('blender.actions',{character:choice.character});if(!current())return;
  const tracks=el('select',{'aria-label':'Animation action',id:'blender-track'},...meta.tracks.map(t=>el('option',{value:t.id,title:t.id},t.label||(choice.character===motion.character?motion.meta?.tracks?.find(a=>a.id===t.id)?.label:null)||t.id.replace(/^@/,'').replaceAll('_',' ').replaceAll('-',' '))));
  if(!tracks.options.length){report('Author a motion track before migrating this rig.');return;}
  if([...tracks.options].some(option=>option.value===choice.track))tracks.value=choice.track;
  choice.track=tracks.value;motion.track=tracks.value;
  const render=el('button',{class:'start',id:'blender-render'},'Render preview');
  const cancel=el('button',{class:'ghost',id:'blender-cancel',disabled:true},'Cancel render');
  const details=el('p',{class:'muted'},'Editable Blender scene and transparent clip are saved with the project.');
  const source=el('select',{id:'blender-source','aria-label':'Animation source'},
   el('option',{value:'saved'},'Saved Blender scene'),el('option',{value:'legacy'},'Reimport legacy action'));
  const range=el('select',{id:'blender-range','aria-label':'Render length'},
   el('option',{value:'90'},'Preview · 90 frames max'),el('option',{value:'18000'},'Full · 18,000 frames max'));
  const resolution=el('select',{id:'blender-resolution','aria-label':'Render resolution'},
   ...[25,50,100].map(value=>el('option',{value:String(value)},`${value}% resolution`)));
  resolution.value='50';
  for(const input of [picker,tracks,source,range,resolution]){
   const tooltip=()=>{input.title=input.selectedOptions[0]?.textContent||'';};
   input.addEventListener('change',tooltip);tooltip();
  }
  const placement=blenderPlacementControls({project,getJob:()=>latest,current});
  const busy=value=>{placement.setBusy(value);render.disabled=value;cancel.disabled=!value;for(const input of [picker,tracks,source,range,resolution])input.disabled=value;};
  controls.append(el('label',{},'Action',tracks),el('label',{},'Source',source),
   el('label',{},'Length',range),el('label',{},'Resolution',resolution),render,cancel,details,placement.node);
  placement.load();
  async function poll(){
   if(!current()||!job)return;
   const requestedJob=job,selectedRevision=selectionRevision;
   const selected=()=>current()&&job===requestedJob&&selectionRevision===selectedRevision;
   try{
    const value=await motionCommand('blender.status',{job});if(!selected())return;
    jobState=value.state;
    if(!['completed','cancelled','failed'].includes(jobState)){
     busy(true);cancel.disabled=value.state==='cancelling';
     report(value.state==='cancelling'?'Cancelling render…':value.progress?.total?`Rendering ${value.progress.completed} / ${value.progress.total}`:'Preparing animation…');
     blenderPollTimer=setTimeout(poll,1000);return;
    }
    busy(false);
    if(value.state==='completed'){
     latest=value;placement.refresh();source.options[0].disabled=false;source.value='saved';source.dispatchEvent(new Event('change'));await blenderLoadPreview(video,value.clip,selected);if(!selected())return;
     details.textContent='Editable Blender scene saved';details.title=value.blend;
     report(value.partial?'Preview ready · shortened clip':'Preview ready');
    }else{
     const failure=value.state==='cancelled'?'Render cancelled':'Render failed';
     if(!latest&&(value.state==='failed'||value.state==='cancelled')){
      const saved=await motionCommand('blender.list');if(!selected())return;
      const matching=saved.jobs.filter(j=>j.character===choice.character&&j.track===tracks.value);
      const index=matching.findIndex(j=>j.job===requestedJob);
      for(const previous of index<0?[]:matching.slice(index+1)){
       const result=await motionCommand('blender.status',{job:previous.job});if(!selected())return;
       if(result.state==='completed'&&result.blend){
        latest=result;placement.refresh();source.options[0].disabled=false;source.value='saved';source.dispatchEvent(new Event('change'));
        details.textContent='Earlier successful Blender scene available';details.title=result.blend;
        await blenderLoadPreview(video,result.clip,selected);if(!selected())return;
        break;
       }
      }
     }
     report(`${failure}${latest?.blend?' · previous saved scene available':''}`,
      value.state==='cancelled'?failure:`${failure} · ${value.error||value.detail||'Inspect the worker log'}`);
    }
   }catch(error){if(!selected())return;const active=!['completed','cancelled','failed'].includes(jobState);busy(active);cancel.disabled=!active||jobState==='cancelling';report(`${error}${active?' · Retrying status…':''}`);if(active)blenderPollTimer=setTimeout(poll,1000);}
  }
  async function restore(){
   clearTimeout(blenderPollTimer);const requestedSelection=++selectionRevision;
   blenderMediaFetch?.abort();
   if(blenderMediaUrl)URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;
   job=null;jobState=null;latest=null;placement.refresh();busy(true);cancel.disabled=true;video.pause();video.removeAttribute('src');video.load();choice.track=tracks.value;
   const selected=tracks.value;
   const saved=await motionCommand('blender.list');if(!current()||requestedSelection!==selectionRevision)return;
   job=saved.jobs.find(j=>j.character===choice.character&&j.track===selected)?.job;
   if(job){cancel.disabled=false;await poll();}else{busy(false);report('Ready to create an editable Blender animation');}
   if(!current()||requestedSelection!==selectionRevision)return;
   source.options[0].disabled=!latest?.blend;source.value=latest?.blend?'saved':'legacy';source.dispatchEvent(new Event('change'));
  }
  tracks.onchange=()=>restore().catch(error=>{if(current()){busy(false);report(String(error));}});
  render.onclick=async()=>{
   ++selectionRevision;clearTimeout(blenderPollTimer);blenderMediaFetch?.abort();
   busy(true);cancel.disabled=true;report('Submitting animation…');
   try{
    const value=await motionCommand('blender.start',{character:choice.character,track:tracks.value,
     ...(source.value==='saved'&&latest?.blend?{blend:latest.blend}:{}),max_frames:Number(range.value),resolution_percent:Number(resolution.value)});
    if(!current())return;job=value.job;jobState=null;cancel.disabled=false;poll();
   }catch(error){if(!current())return;busy(false);report(String(error));}
  };
  cancel.onclick=async()=>{cancel.disabled=true;try{await motionCommand('blender.cancel',{job});if(current()){clearTimeout(blenderPollTimer);await poll();}}catch(error){if(current())cancel.disabled=false;report(String(error));}};
  await restore().catch(error=>{if(current())busy(false);throw error;});
 }catch(error){report(String(error));}
}
