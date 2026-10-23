// Stock Theatre UI is contained inside the current Animation surface, so it
// cannot cover the native inspector or Sproket workstation.
const magicTheatre={frame:null,key:null,part:null,rotation:0,ready:false,syncing:false,availability:'unavailable',error:null,objects:[],playing:null,position:null,valueUpdate:null,project:null,load:null,savedAt:null,persistence:'not_loaded',exports:new Map(),nextRequest:0};
function theatreFeedback(){
 return {availability:magicTheatre.availability,package_version:'0.7.0',error:magicTheatre.error,selected_objects:magicTheatre.objects,native_selected_part:motion.selected,sequence_position_seconds:magicTheatre.position,sequence_playing:magicTheatre.playing,native_playing:motion.playing,value_update:magicTheatre.valueUpdate,persistence:magicTheatre.persistence,saved_snapshot_unix_ms:magicTheatre.savedAt,live_edits:'not_assessed',visual_quality:'not_assessed'};
}
function theatreEvent(stage,detail={}){
 if(typeof acFeedback!=='undefined')acFeedback.report(null,null,{stage:`theatre_${stage}`,...detail});
}
function theatreMount(container){
 const status=el('span',{id:'theatre-status',class:'muted'},'Loading Theatre…');
 const frame=el('iframe',{id:'theatre-timeline',title:'Theatre motion timeline',src:'theatre-frame.html'});
 Object.assign(magicTheatre,{frame,key:null,ready:false,rotation:0,availability:'loading',error:null,objects:[],playing:null,position:null,valueUpdate:null,project:null,load:null,savedAt:null,persistence:'not_loaded'});
 const save=el('button',{id:'theatre-save',disabled:true,title:'Save native Theatre keyframes and curves into the current project',onclick:()=>theatreSave().catch(error=>toast(String(error)))},'Save motion');
 container.append(el('div',{class:'theatre-heading'},el('strong',{},'Theatre'),status,save),frame);
}
async function theatreContext(){
 if(!magicTheatre.ready||!motion.meta)return;
 const part=motion.selected||motion.meta.layers[0]?.id;if(!part)return;
 const key=JSON.stringify([state.project,motion.character,part]);if(key===magicTheatre.key)return;
 magicTheatre.key=key;magicTheatre.part=part;magicTheatre.rotation=0;
 const project=state.project,character=motion.character,frame=magicTheatre.frame;
 try{
  if(magicTheatre.project!==project){magicTheatre.project=project;magicTheatre.load=motionCommand('motion.theatre.read',{expected_project:project});}
  const loaded=await magicTheatre.load;
  if(frame!==magicTheatre.frame||key!==magicTheatre.key||project!==state.project)return;
  magicTheatre.persistence=loaded.status;
  frame.contentWindow.postMessage({magicTheatre:true,type:'context',project,character,part,state:loaded.state},location.origin);
 }catch(error){magicTheatre.availability='error';magicTheatre.error=String(error);$('theatre-status').textContent=`Theatre load failed: ${error}`;theatreEvent('error',{error:String(error)});}
}
async function theatreSave(){
 if(magicTheatre.availability!=='ready')throw new Error('Theatre is not ready to save');
 const project=state.project,frame=magicTheatre.frame,request=++magicTheatre.nextRequest,button=$('theatre-save');button.disabled=true;
 try{
  const exported=await new Promise((resolve,reject)=>{
   const timer=setTimeout(()=>{magicTheatre.exports.delete(request);reject(new Error('Theatre export timed out; no project save was submitted'));},5000);
   magicTheatre.exports.set(request,{project,resolve,reject,timer});
   frame.contentWindow.postMessage({magicTheatre:true,type:'export',request,project},location.origin);
  });
  if(state.project!==project||magicTheatre.frame!==frame)throw new Error('Project changed during export; no state saved');
  const result=await motionCommand('motion.theatre.save',{expected_project:project,state:exported.state});
  if(result.saved!==true||result.project!==project)throw new Error('Native Theatre save was not acknowledged');
  magicTheatre.savedAt=Date.now();magicTheatre.persistence='saved_snapshot';$('theatre-status').textContent=`${magicTheatre.part} · motion snapshot saved`;theatreEvent('saved',{project,saved_unix_ms:magicTheatre.savedAt});return result;
 }finally{if(frame===magicTheatre.frame)button.disabled=false;}
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
 if(message.type==='loaded'){magicTheatre.ready=true;theatreContext();}
 if(message.type==='ready'){magicTheatre.availability='ready';magicTheatre.error=null;$('theatre-save').disabled=false;$('theatre-status').textContent=`${message.part} · ${magicTheatre.persistence==='saved'?'saved motion loaded':'Save motion to persist edits'}`;theatreEvent('ready',{part:message.part});}
 if(message.type==='exported'||message.type==='error'&&message.request){const pending=magicTheatre.exports.get(message.request);if(pending){clearTimeout(pending.timer);magicTheatre.exports.delete(message.request);if(message.type==='error'||message.project!==pending.project)pending.reject(new Error(message.error||'Theatre export project mismatch'));else pending.resolve(message);}}
 if(message.type==='selection'){magicTheatre.objects=message.objects;theatreEvent('selection',{objects:message.objects});}
 if(message.type==='playback'){const changed=magicTheatre.playing!==message.playing;magicTheatre.playing=message.playing;if(changed)theatreEvent('playback',{playing:message.playing});}
 if(message.type==='values'&&message.part===magicTheatre.part){magicTheatre.rotation=message.rotation;theatreRender();magicTheatre.valueUpdate={part:message.part,property:'rotation',value:message.rotation,received_unix_ms:Date.now(),render_call_completed:true};}
 if(message.type==='seek'&&motion.sample){animationPause();motion.time=Math.max(0,Math.min(message.time,motion.sample.duration));magicTheatre.position=message.time;theatreRender();}
 if(message.type==='error'){magicTheatre.availability='error';magicTheatre.error=String(message.error);$('theatre-status').textContent=`Theatre failed: ${message.error}`;theatreEvent('error',{error:magicTheatre.error});}
});
function theatreTransform(ctx,layer){
 if(layer.id!==magicTheatre.part||!magicTheatre.rotation)return;
 const pivot=layer.pivot||[0,0];
 ctx.translate(pivot[0],pivot[1]);ctx.rotate(magicTheatre.rotation*Math.PI/180);ctx.translate(-pivot[0],-pivot[1]);
}
