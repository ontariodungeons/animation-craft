// Blender owns animation data; this surface only chooses jobs and plays rendered media.
let blenderViewRevision=0;
let blenderMediaUrl=null,blenderMediaFetch=null,blenderPollTimer=null;
function blenderDispose(){
 clearTimeout(blenderPollTimer);blenderPollTimer=null;
 ++blenderViewRevision;blenderMediaFetch?.abort();blenderMediaFetch=null;
 const video=document.querySelector('#view-animation video');
 if(video){video.pause();video.removeAttribute('src');video.load();}
 if(blenderMediaUrl)URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;
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
 blenderMediaUrl=URL.createObjectURL(new Blob(chunks,{type:'video/webm'}));
 video.src=blenderMediaUrl;video.load();
}
async function blenderAnimationOpen(){
 blenderDispose();
 theatreRequireSaved();
 animationPause();motion.resizeObserver?.disconnect();motion.images=[];motion.sample=null;motion.request++;
 for(const pending of magicTheatre.exports.values()){clearTimeout(pending.timer);pending.reject(new Error('Animation editor closed'));}
 magicTheatre.exports.clear();
 Object.assign(magicTheatre,{frame:null,load:null,key:null,sheet:null,ready:false,project:null,stateSentProject:null,availability:'unavailable',rotations:{},sequenced:{}});
 const revision=++blenderViewRevision,project=state.project;
 const current=()=>revision===blenderViewRevision&&state.project===project&&state.view==='animation'&&motion.mode==='blender';
 const view=$('view-animation');view.replaceChildren();
 const controls=el('aside',{class:'motion-inspector blender-controls',id:'blender-controls','aria-label':'Blender animation controls'});
 const stage=el('div',{class:'motion-stage'});
 const status=el('span',{class:'muted',role:'status'},'Loading artwork…');
 const main=el('div',{class:'motion-main'},el('div',{class:'motion-stagebar'},motionModeSwitch(),status),stage);
 view.append(controls,main);
 const video=el('video',{id:'blender-preview',controls:true,loop:true,'aria-label':'Rendered Blender animation'});
 video.style.cssText='width:100%;height:100%;object-fit:contain';
 stage.append(video);
 let latest=null,job=null,selectionRevision=0;
 const report=message=>{if(current())status.textContent=message;};
 try{
  const result=await motionCommand('project.list_assets');if(!current())return;
  const assets=result.assets||result.project_assets||[];
  const candidates=assets.filter(a=>a.kind==='cutout_rig');
  if(!candidates.some(a=>a.id===motion.character))motion.character=candidates[0]?.id;
  if(!motion.character){report('Choose a rigged character in Project to begin.');return;}
  const picker=el('select',{'aria-label':'Animated object',id:'blender-object'},...candidates.map(a=>el('option',{value:a.id},a.name||a.id)));
  picker.value=motion.character;
  picker.onchange=()=>{motion.character=picker.value;blenderAnimationOpen();};
  controls.append(picker);
  const meta=await motionCommand('motion.tracks',{character:motion.character});if(!current())return;
  motion.meta=meta;motion.project=project;updateMagicSelection();
  motion.rest=false;
  const tracks=el('select',{'aria-label':'Animation action',id:'blender-track'},...meta.tracks.filter(t=>!t.standard).map(t=>el('option',{value:t.id},t.id)));
  if(!tracks.options.length){report('Author a motion track before migrating this rig.');return;}
  if([...tracks.options].some(option=>option.value===motion.track))tracks.value=motion.track;
  motion.track=tracks.value;
  const render=el('button',{class:'start',id:'blender-render'},'Render preview');
  const cancel=el('button',{class:'ghost',id:'blender-cancel',disabled:true},'Cancel render');
  const details=el('p',{class:'muted'},'Editable Blender scene and transparent clip are saved with the project.');
  const source=el('select',{id:'blender-source','aria-label':'Animation source'},
   el('option',{value:'saved'},'Saved Blender scene'),el('option',{value:'legacy'},'Reimport legacy action'));
  const range=el('select',{id:'blender-range','aria-label':'Render length'},
   el('option',{value:'90'},'Quick preview · up to 90 frames'),el('option',{value:'18000'},'Full action · up to 18,000 frames'));
  const resolution=el('select',{id:'blender-resolution','aria-label':'Render resolution'},
   ...[25,50,100].map(value=>el('option',{value:String(value)},`${value}% resolution`)));
  resolution.value='50';
  const busy=value=>{render.disabled=value;cancel.disabled=!value;for(const input of [picker,tracks,source,range,resolution])input.disabled=value;};
  controls.append(el('label',{},'Action',tracks),el('label',{},'Source',source),
   el('label',{},'Length',range),el('label',{},'Resolution',resolution),render,cancel,details);
  async function poll(){
   if(!current()||!job)return;
   const requestedJob=job,selectedRevision=selectionRevision;
   const selected=()=>current()&&job===requestedJob&&selectionRevision===selectedRevision;
   try{
    const value=await motionCommand('blender.status',{job});if(!selected())return;
    if(value.state==='running'){
     report(value.progress?.total?`Rendering ${value.progress.completed} / ${value.progress.total}`:'Preparing animation…');
     blenderPollTimer=setTimeout(poll,1000);return;
    }
    busy(false);
    if(value.state==='completed'){
     latest=value;source.options[0].disabled=false;source.value='saved';await blenderLoadPreview(video,value.clip,selected);if(!selected())return;
     details.textContent='Editable Blender scene saved';details.title=value.blend;
     report(value.partial?'Preview ready · shortened clip':'Preview ready');
    }else report(value.state==='cancelled'?'Render cancelled':`Render failed · ${value.error||value.detail||'inspect the worker log'}`);
   }catch(error){if(!selected())return;busy(false);report(String(error));}
  }
  async function restore(){
   clearTimeout(blenderPollTimer);const requestedSelection=++selectionRevision;
   blenderMediaFetch?.abort();
   if(blenderMediaUrl)URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;
   job=null;latest=null;busy(true);cancel.disabled=true;video.pause();video.removeAttribute('src');video.load();motion.track=tracks.value;
   const selected=tracks.value;
   const saved=await motionCommand('blender.list');if(!current()||requestedSelection!==selectionRevision)return;
   job=saved.jobs.find(j=>j.character===motion.character&&j.track===selected)?.job;
   if(job){cancel.disabled=false;await poll();}else{busy(false);report('Ready to create an editable Blender animation');}
   if(!current()||requestedSelection!==selectionRevision)return;
   source.options[0].disabled=!latest?.blend;source.value=latest?.blend?'saved':'legacy';
  }
  tracks.onchange=()=>restore().catch(error=>{if(current()){busy(false);report(String(error));}});
  render.onclick=async()=>{
   ++selectionRevision;clearTimeout(blenderPollTimer);blenderMediaFetch?.abort();
   busy(true);cancel.disabled=true;report('Submitting animation…');
   try{
    const value=await motionCommand('blender.start',{character:motion.character,track:tracks.value,
     ...(source.value==='saved'&&latest?.blend?{blend:latest.blend}:{}),max_frames:Number(range.value),resolution_percent:Number(resolution.value)});
    if(!current())return;job=value.job;cancel.disabled=false;poll();
   }catch(error){if(!current())return;busy(false);report(String(error));}
  };
  cancel.onclick=async()=>{cancel.disabled=true;try{await motionCommand('blender.cancel',{job});if(current()){clearTimeout(blenderPollTimer);await poll();}}catch(error){if(current())cancel.disabled=false;report(String(error));}};
  await restore().catch(error=>{if(current())busy(false);throw error;});
 }catch(error){report(String(error));}
}
