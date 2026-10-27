// Stock Theatre UI is contained inside the current Animation surface, so it
// cannot cover the native inspector or Sproket workstation.
const magicTheatre={frame:null,key:null,sheet:null,part:null,rotation:0,rotations:{},ready:false,syncing:false,availability:'unavailable',error:null,objects:[],playing:null,position:null,valueUpdate:null,project:null,stateSentProject:null,load:null,savedAt:null,persistence:'not_loaded',exports:new Map(),nextRequest:0,interactionRevision:0,savedRevision:0,gestureActive:false};
function theatreFeedback(){
 return {availability:magicTheatre.availability,package_version:'0.7.0',error:magicTheatre.error,selected_objects:magicTheatre.objects,native_selected_part:motion.selected,sequence_position_seconds:magicTheatre.position,sequence_playing:magicTheatre.playing,native_playing:motion.playing,value_update:magicTheatre.valueUpdate,persistence:magicTheatre.persistence,saved_snapshot_unix_ms:magicTheatre.savedAt,live_edits:magicTheatre.interactionRevision>magicTheatre.savedRevision||magicTheatre.gestureActive?'needs_save':magicTheatre.savedAt||magicTheatre.persistence==='saved'?'saved_snapshot':'no_observed_interaction',visual_quality:'not_assessed'};
}
function theatreEvent(stage,detail={}){
 if(typeof acFeedback!=='undefined')acFeedback.report(null,null,{stage:`theatre_${stage}`,...detail});
}
function theatreRequireSaved(){
 if(magicTheatre.interactionRevision>magicTheatre.savedRevision||magicTheatre.gestureActive)throw new Error('Save motion before changing the animation workspace; Theatre interactions may contain unsaved keyframes.');
}
function theatreCanLeave(){
 try{theatreRequireSaved();return true;}catch(error){if(typeof toast==='function')toast(String(error));return false;}
}
function theatreMount(container){
 theatreRequireSaved();
 const status=el('span',{id:'theatre-status',class:'muted'},'Loading Theatre…');
 const clips=el('div',{id:'theatre-clips',class:'theatre-clips','aria-label':'Authored clips'});
 const frame=el('iframe',{id:'theatre-timeline',title:'Theatre motion timeline',src:'theatre-frame.html'});
 Object.assign(magicTheatre,{frame,key:null,sheet:null,ready:false,rotation:0,rotations:{},availability:'loading',error:null,objects:[],playing:null,position:null,valueUpdate:null,project:null,stateSentProject:null,load:null,savedAt:null,persistence:'not_loaded',interactionRevision:0,savedRevision:0,gestureActive:false});
 const save=el('button',{id:'theatre-save',class:'ghost',disabled:true,title:'Save native Theatre keyframes and curves into the current project',onclick:()=>theatreSave().catch(error=>toast(String(error)))},'Save motion');
 // Curves draws the clip's channels natively; Keys shows Theatre's own keyframe editor. The
 // Theatre frame stays mounted under both so its saves and unsaved-edit guards keep working.
 let view='curves';try{view=localStorage.getItem('theatre.view')==='keys'?'keys':'curves';}catch{}
 const views=el('div',{class:'theatre-views',role:'tablist','aria-label':'Theatre view'},...[['keys','Keys','Edit keyframes in Theatre'],['curves','Curves','Every animated channel of this clip over time']].map(([key,label,title])=>
  el('button',{class:`theatre-view${view===key?' active':''}`,id:`theatre-view-${key}`,role:'tab','aria-selected':String(view===key),title,onclick:()=>theatreSetView(key)},label)));
 const icon=el('span',{class:'theatre-icon','aria-hidden':'true',html:'<svg viewBox="0 0 24 24"><path d="M3 3h18v18H3zM3 9h18M3 15h18M9 3v18M15 3v18"/></svg>'});
 const curves=el('div',{class:'theatre-curves',id:'theatre-curves','aria-label':'Animated channels'});
 if(container.dataset)container.dataset.view=view;
 container.append(el('div',{class:'theatre-heading'},icon,el('strong',{},'Theatre'),views,el('span',{id:'theatre-info',class:'theatre-info muted'}),el('div',{class:'theatre-actions'},status,save)),
  el('div',{class:'theatre-cliprow'},el('span',{class:'theatre-label'},'Clips'),clips),
  el('div',{class:'theatre-body'},frame,curves));
 theatreSyncClips();
}
function theatreSetView(view){
 const section=$('theatre-curves')?.closest?.('.motion-theatre');if(!section)return;
 section.dataset.view=view;try{localStorage.setItem('theatre.view',view);}catch{}
 for(const key of ['keys','curves']){const tab=$(`theatre-view-${key}`);tab?.classList.toggle('active',key===view);tab?.setAttribute('aria-selected',String(key===view));}
 if(view==='curves'&&typeof timelineRender==='function')timelineRender();
}
function theatreSyncClips(){
 const bar=$('theatre-clips');if(!bar||typeof bar.replaceChildren!=='function'||!motion.meta?.tracks)return;
 const authored=(motion.meta.tracks||[]).filter(t=>!t.standard);
 const pills=authored.map(t=>{
  const active=!motion.rest&&motion.track===t.id;
  const label=t.id.replace(/^.*?-beat\d+-/,'').replaceAll('-',' ');
  const dur=Number.isFinite(t.duration)?` · ${Number(t.duration).toFixed(2)}s`:'';
  return el('button',{class:`theatre-clip-pill${active?' active':''}`,title:`Open clip sheet ${t.id}`,'aria-pressed':String(active),onclick:async()=>{if(!theatreCanLeave())return;motion.track=t.id;motion.rest=false;motion.time=0;if(typeof acFeedback!=='undefined')acFeedback.report(null,null,{stage:'clip_switched',clip:t.id});if(typeof animationControls==='function')animationControls();if(typeof animationSample==='function')await animationSample();}},`${label}${dur}`);
 });
 pills.push(el('button',{class:`theatre-clip-pill${motion.rest?' active':''}`,title:'Open rest pose sheet','aria-pressed':String(Boolean(motion.rest)),onclick:async()=>{if(!theatreCanLeave())return;motion.rest=true;motion.time=0;if(typeof acFeedback!=='undefined')acFeedback.report(null,null,{stage:'clip_switched',clip:'@rest'});if(typeof animationControls==='function')animationControls();if(typeof animationSample==='function')await animationSample();}},'Rest'));
 bar.replaceChildren(...pills);
 if(typeof timelineRender==='function')timelineRender();
}
async function theatreContext(){
 if(!magicTheatre.ready||!motion.meta)return;
 const part=motion.selected||motion.meta.layers[0]?.id;if(!part)return;
 // One Theatre sheet per clip, the same "<asset> / <clip>" id the engine's motion.clips uses.
 const sheet=`${motion.character} / ${(motion.rest?'@rest':motion.track||'@rest').replace(/^@/,'')}`;
 const parts=(motion.meta.layers||[]).map(l=>l.id);
 const key=JSON.stringify([state.project,motion.character,part,sheet]);if(key===magicTheatre.key)return;
 if(magicTheatre.sheet!==sheet){magicTheatre.sheet=sheet;magicTheatre.rotations={};}
 magicTheatre.key=key;magicTheatre.part=part;magicTheatre.rotation=magicTheatre.rotations[part]||0;
 const project=state.project,character=motion.character,frame=magicTheatre.frame;
 try{
  if(magicTheatre.project!==project){magicTheatre.project=project;magicTheatre.load=motionCommand('motion.theatre.read',{expected_project:project});}
  const loaded=await magicTheatre.load;
  if(frame!==magicTheatre.frame||key!==magicTheatre.key||project!==state.project)return;
  magicTheatre.persistence=loaded.status;
  const statePayload=magicTheatre.stateSentProject===project?undefined:loaded.state;
  magicTheatre.stateSentProject=project;
  frame.contentWindow.postMessage({magicTheatre:true,type:'context',project,character,sheet,part,parts,state:statePayload},location.origin);
 }catch(error){magicTheatre.availability='error';magicTheatre.error=String(error);$('theatre-status').textContent=`Theatre load failed: ${error}`;theatreEvent('error',{error:String(error)});}
}
async function theatreSave(){
 if(magicTheatre.availability!=='ready')throw new Error('Theatre is not ready to save');
 const project=state.project,frame=magicTheatre.frame,request=++magicTheatre.nextRequest,revision=magicTheatre.interactionRevision,button=$('theatre-save');button.disabled=true;
 try{
  const exported=await new Promise((resolve,reject)=>{
   const timer=setTimeout(()=>{magicTheatre.exports.delete(request);reject(new Error('Theatre export timed out; no project save was submitted'));},5000);
   magicTheatre.exports.set(request,{project,resolve,reject,timer});
   frame.contentWindow.postMessage({magicTheatre:true,type:'export',request,project},location.origin);
  });
  if(state.project!==project||magicTheatre.frame!==frame)throw new Error('Project changed during export; no state saved');
  // An untouched timeline exports no sheets; convert the character's clip sheets so the timeline snapshot persists cleanly.
  if(!Object.keys(exported.state?.sheetsById||{}).length){
   if(motion.character){
    const converted=await motionCommand('motion.clips.convert',{character:motion.character});
    if(magicTheatre.interactionRevision===revision&&!magicTheatre.gestureActive)magicTheatre.savedRevision=revision;
    magicTheatre.savedAt=Date.now();magicTheatre.persistence='saved_snapshot';$('theatre-status').textContent=`${magicTheatre.part} · motion timeline saved`;theatreEvent('saved',{project,saved_unix_ms:magicTheatre.savedAt});if(typeof animationSample==='function')await animationSample();return converted;
   }
   if(magicTheatre.interactionRevision===revision&&!magicTheatre.gestureActive)magicTheatre.savedRevision=revision;$('theatre-status').textContent=`${magicTheatre.part} · no Theatre edits to save`;return {saved:false,reason:'no Theatre edits'};
  }
  const result=await motionCommand('motion.theatre.save',{expected_project:project,state:exported.state});
  if(result.saved!==true||result.project!==project)throw new Error('Native Theatre save was not acknowledged');
  if(state.project!==project||magicTheatre.frame!==frame)throw new Error('Project changed during save; the previous project snapshot was saved');
  if(magicTheatre.interactionRevision===revision&&!magicTheatre.gestureActive)magicTheatre.savedRevision=revision;
  magicTheatre.savedAt=Date.now();magicTheatre.persistence='saved_snapshot';$('theatre-status').textContent=`${magicTheatre.part} · motion snapshot saved`;theatreEvent('saved',{project,saved_unix_ms:magicTheatre.savedAt});if(typeof animationSample==='function')await animationSample();return result;
 }finally{if(frame===magicTheatre.frame)button.disabled=false;if(typeof updateAppFooter==='function')updateAppFooter();}
}
function theatreSeek(time){
 theatreContext();
 if(magicTheatre.ready&&!magicTheatre.syncing){magicTheatre.position=time;magicTheatre.frame.contentWindow.postMessage({magicTheatre:true,type:'seek',time},location.origin);}
}
function theatreRender(){
 const previous=magicTheatre.syncing;magicTheatre.syncing=true;
 try{animationDraw();}finally{magicTheatre.syncing=previous;}
}
window.addEventListener('message',event=>{
 if(event.source!==magicTheatre.frame?.contentWindow||!event.data?.magicTheatre)return;
 const message=event.data;
 if(message.type==='interaction'&&message.project===state.project&&message.project===magicTheatre.project&&Number.isSafeInteger(message.revision)&&message.revision>magicTheatre.interactionRevision){magicTheatre.interactionRevision=message.revision;magicTheatre.gestureActive=message.active===true;if(typeof acFeedback!=='undefined')acFeedback.report();}
 if(message.type==='loaded'){magicTheatre.ready=true;theatreContext();}
 if(message.type==='ready'){magicTheatre.availability='ready';magicTheatre.error=null;$('theatre-save').disabled=false;$('theatre-status').textContent=`${message.part} · ${magicTheatre.persistence==='saved'?'saved motion loaded':'Save motion to persist edits'}`;theatreEvent('ready',{part:message.part});}
 if(message.type==='exported'||message.type==='error'&&message.request){const pending=magicTheatre.exports.get(message.request);if(pending){clearTimeout(pending.timer);magicTheatre.exports.delete(message.request);if(message.type==='error'||message.project!==pending.project)pending.reject(new Error(message.error||'Theatre export project mismatch'));else pending.resolve(message);}}
 if(message.type==='keyframes'){theatreEvent('keyframes',{sheet:message.sheet,part:message.part,count:message.count,previous:message.previous});}
 if(message.type==='selection'){magicTheatre.objects=message.objects;const picked=message.objects?.[0]?.part;if(picked&&motion.meta?.layers?.some(l=>l.id===picked)&&motion.selected!==picked){motion.selected=picked;magicTheatre.part=picked;magicTheatre.rotation=magicTheatre.rotations[picked]||0;if(typeof updateMagicSelection==='function')updateMagicSelection();}theatreEvent('selection',{objects:message.objects});}
 if(message.type==='playback'){const changed=magicTheatre.playing!==message.playing;magicTheatre.playing=message.playing;if(changed)theatreEvent('playback',{playing:message.playing});}
 if(message.type==='values'){
  if(motion.meta?.layers?.some(l=>l.id===message.part))magicTheatre.rotations[message.part]=message.rotation;
  if(message.part===magicTheatre.part){magicTheatre.rotation=message.rotation;if(!message.sync)theatreRender();magicTheatre.valueUpdate={part:message.part,property:'rotation',value:message.rotation,received_unix_ms:Date.now(),render_call_completed:true};}
  else if(motion.meta?.layers?.some(l=>l.id===message.part)&&!message.sync){theatreRender();}
 }
 if(message.type==='seek'&&motion.sample){animationPause();motion.time=Math.max(0,Math.min(message.time,motion.sample.duration));magicTheatre.position=message.time;theatreRender();}
 if(message.type==='error'){magicTheatre.availability='error';magicTheatre.error=String(message.error);$('theatre-status').textContent=`Theatre failed: ${message.error}`;theatreEvent('error',{error:message.error});}
});
// Live unsaved Theatre gestures apply on top of the engine sample; saved Theatre keyframes are already baked into motion.sample.
function theatreTransform(ctx,layer){
 if(!(magicTheatre.interactionRevision>magicTheatre.savedRevision||magicTheatre.gestureActive))return;
 const effective=layer.id===magicTheatre.part?magicTheatre.rotation:(magicTheatre.rotations?.[layer.id]||0);
 if(!effective)return;
 const pivot=layer.pivot||[0,0];
 ctx.translate(pivot[0],pivot[1]);ctx.rotate(effective*Math.PI/180);ctx.translate(-pivot[0],-pivot[1]);
}
