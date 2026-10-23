// Persisted rig edits use shared commands and optimistic rig hashes, never preview-only transforms.
async function animationTheatreSnapshot(){
 theatreRequireSaved();
 if(magicTheatre.availability!=='ready'||!magicTheatre.ready||!magicTheatre.frame)throw Error(magicTheatre.error||'Wait for the native timeline before capturing a pose.');
 const project=state.project,frame=magicTheatre.frame,request=++magicTheatre.nextRequest;
 const value=await new Promise((resolve,reject)=>{const timer=setTimeout(()=>{magicTheatre.exports.delete(request);reject(Error('Timeline snapshot timed out; no pose exported.'));},5000);magicTheatre.exports.set(request,{project,resolve,reject,timer});frame.contentWindow.postMessage({magicTheatre:true,type:'export',request,project},location.origin);});
 if(state.project!==project||magicTheatre.frame!==frame)throw Error('Project changed during timeline capture.');
 theatreRequireSaved();return value.state;
}
function motionAuthoringPanels(meta){
 const project=state.project,character=motion.character,track=motion.track,part=meta.layers.find(l=>l.id===motion.selected),action=meta.tracks.find(t=>t.id===track),caps=action?.authoring||{},hash=meta.rig_sha256;
 const current=()=>state.project===project&&motion.character===character;
 const identity=()=>({expected_project:project,character,track,expected_rig_sha256:hash});
 const field=(id,label,value,type='number')=>{const input=el('input',{id:`authoring-${id}`,type,value:value??'',step:'any',placeholder:value===null?'Animated':undefined,'aria-label':label});return {input,node:el('label',{},label,input)};};
 const status=el('p',{id:'authoring-edit-status',role:'status',class:'muted'}),readback=el('pre',{class:'animation-readback',id:'authoring-edit-result'}),cli=el('code',{class:'animation-readback',id:'authoring-command'});
 const reason=(id,allowed)=>el('p',{id:`authoring-${id}-reason`,class:'muted'},allowed?'Changes replace only the edited action channels. Save project separately.':caps.reason||'A project-local authored action and authoring backend are required.');
 const editControls=[];
 const key=`animation.editUndo.${project}.${character}`;
 const quote=v=>`'${String(v).replaceAll("'","'\\''")}'`;
 const command=(id,params)=>`mc exec ${id} ${quote(`project=${project}`)} ${Object.entries(params).map(([k,v])=>quote(`${k}=${typeof v==='object'?JSON.stringify(v):v}`)).join(' ')}`;
 async function edit(id,params){
  if(motion.authoringBusy)throw Error('Wait for the current rig edit.');
  theatreRequireSaved();if(!current())throw Error('Selected project or character changed.');
  motion.authoringBusy=true;for(const control of editControls)control.disabled=true;
  try{
  cli.textContent=command(id,params);status.textContent='Applying…';
  const result=await motionCommand(id,params);
  if(current())readback.textContent=JSON.stringify(result,null,2);
  if(result.ok!==true||result.character!==character||!result.rig_sha256||!result.undo)throw Error('Edit returned no hash/undo confirmation; inspect result.');
  if(!current())return;
  motion.lastEdit={project,character,track,command:id,result};localStorage.setItem(key,JSON.stringify({undo:result.undo,hash:result.rig_sha256}));readback.textContent=JSON.stringify(result,null,2);status.textContent='Rig saved · undo available · project Save remains explicit.';
  motion.authoringBusy=false;
  try{await animationRefresh();}catch(error){motion.lastEdit.refresh_error=String(error);status.textContent=`Rig edit succeeded; refresh failed: ${error}`;}
  }finally{motion.authoringBusy=false;for(const control of editControls)control.disabled=control.unavailable===true;}
 }
 const run=(id,params)=>edit(id,params).catch(error=>{if(current()){status.textContent=String(error);status.className='image-error';}});
 const dirty=new Set(),values={},partRows=[];
 for(const [property,label,count]of [['position','Position offset (px)',2],['scale','Scale (%)',2],['rotation','Rotation (degrees)',1],['pivot','Pivot (canvas px)',2]]){
  const v=property==='pivot'?part?.pivot:action?.static_overrides?.[part?.id]?.[property];
  const fields=Array.from({length:count},(_,i)=>field(count===1?property:`${property}-${i}`,count===1?label:`${label} ${i?'Y':'X'}`,count===1?v:v?.[i]??null));values[property]=fields;
  for(const f of fields)f.input.onchange=()=>dirty.add(property);
  partRows.push(el('div',{class:count===2?'motion-coordinate-row':''},...fields.map(f=>f.node)));
 }
 function partRequest(){if(!part)throw Error('Select a part.');if(!dirty.size)throw Error('Change a part value first.');const p={...identity(),part:part.id};for(const k of dirty){const v=values[k].map(f=>{if(!String(f.input.value).trim()||!Number.isFinite(Number(f.input.value)))throw Error(`Enter every ${k} coordinate; animated channels need an explicit constant.`);return Number(f.input.value)});p[k]=v.length===1?v[0]:v;}return p;}
 const apply=el('button',{id:'authoring-part-apply',class:'start','data-commands':'motion.part.set',onclick:()=>{try{return run('motion.part.set',partRequest())}catch(error){status.textContent=String(error)}}},'Apply part values');apply.disabled=!part||caps.part_set!==true;
 const reset=el('button',{id:'authoring-part-reset',class:'ghost','data-commands':'motion.part.reset',onclick:()=>run('motion.part.reset',{...identity(),part:part.id,fields:['position','scale','rotation','pivot']})},'Reset part');reset.disabled=!part||caps.part_reset!==true;
 const undo=el('button',{id:'authoring-undo',class:'ghost','data-commands':'motion.edit.undo',onclick:()=>{try{const saved=JSON.parse(localStorage.getItem(key)||'null');if(!saved)throw Error('No saved undo receipt.');return run('motion.edit.undo',{...identity(),expected_rig_sha256:hash,undo:saved.undo});}catch(error){status.textContent=String(error)}}},'Undo rig edit');undo.disabled=!localStorage.getItem(key);undo.title='Undo is checked against the current rig hash; Theatre edits can block it.';
 const partNode=el('div',{class:'animation-authoring-fields'},el('strong',{},part?.label||part?.id||'Select a part'),reason('part',!!part&&caps.part_set===true),...partRows,apply,reset,undo,el('details',{},el('summary',{},'Edit receipt and exact CLI'),cli,readback));
 if(motion.lastEdit?.project===project&&motion.lastEdit.character===character){readback.textContent=JSON.stringify(motion.lastEdit,null,2);status.textContent='Rig saved · undo receipt available · project Save remains explicit.';}
 const label=field('label','Action name',action?.label||action?.id||'','text'),duration=field('duration','Action duration (seconds)',action?.duration),loop=el('input',{id:'authoring-loop',type:'checkbox','aria-label':'Loop authored action'});loop.checked=action?.loop===true;loop.defaultChecked=loop.checked;
 const actionDirty=new Set();label.input.onchange=()=>actionDirty.add('label');duration.input.onchange=()=>actionDirty.add('duration');loop.onchange=()=>actionDirty.add('loop');
 const actionApply=el('button',{id:'authoring-action-apply',class:'ghost','data-commands':'motion.action.set',onclick:()=>{try{if(!actionDirty.size)throw Error('Change an action value first.');const p=identity();for(const k of actionDirty){if(k==='label'){if(!label.input.value.trim())throw Error('Enter an action name.');p.label=label.input.value.trim();}else if(k==='duration'){const d=Number(duration.input.value);if(!Number.isFinite(d)||d<=0||d>600)throw Error('Duration must be in (0,600].');p.duration=d;}else p.loop=loop.checked;}return run('motion.action.set',p);}catch(error){status.textContent=String(error)}}},'Save action settings');actionApply.disabled=caps.action_set!==true;
 const actionNode=el('div',{class:'animation-authoring-fields'},reason('action',caps.action_set===true),label.node,duration.node,el('label',{},loop,'Loop authored action'),actionApply);
 const posePreview=el('img',{id:'authoring-pose-preview',alt:'Captured static pose pending approval',class:'animation-pose-preview'});posePreview.hidden=true;
 const poseStatus=el('p',{id:'authoring-pose-status',role:'status',class:'muted'}),poseResult=el('pre',{class:'animation-readback',id:'authoring-pose-result'}),reference=el('select',{id:'authoring-reference','aria-label':'Optional registered pose reference'},el('option',{value:''},'Source provenance only'),...(motion.assets||[]).filter(a=>a.kind==='image').map(a=>el('option',{value:a.id},a.name||a.id)));
 const capture=el('button',{id:'authoring-pose-export',class:'ghost','data-commands':'motion.pose.export',onclick:async()=>{
  capture.disabled=true;try{if(motion.authoringBusy)throw Error('Wait for the current rig edit.');theatreRequireSaved();if(typeof animationPause==='function')animationPause();const params={...identity(),time:motion.time,rest:motion.rest===true,amount:motion.amount??100,...(reference.value?{reference_asset:reference.value}:{})},snapshot=await animationTheatreSnapshot();if(!current())return;const result=await motionCommand('motion.pose.export',{...params,theatre_state:snapshot});if(!current())return;if(result.ok!==true||!result.asset_id||!result.receipt||result.approval!=='pending')throw Error('Pose returned incomplete pending-asset receipt.');motion.lastPose={project,character,track,result};if(result.path){posePreview.src=convertFileSrc(result.path);posePreview.hidden=false;}poseResult.textContent=JSON.stringify(result,null,2);poseStatus.textContent='Pose registered in memory · pending approval. Save project explicitly to retain registration.';await animationRefresh();}catch(error){if(current())poseStatus.textContent=`${motion.lastPose?.result?'Pose receipt retained. ':''}${error}`;}finally{capture.disabled=caps.pose_export!==true;}
 }} ,'Capture pending pose');capture.disabled=caps.pose_export!==true;
 const scenes=el('select',{id:'authoring-pose-scene','aria-label':'Pose destination scene'}),x=field('pose-x','Storyboard X',0),y=field('pose-y','Storyboard Y',0),width=field('pose-width','Storyboard width',160);
 const place=el('button',{id:'authoring-pose-place',class:'ghost','data-commands':'storyboard.place_asset',onclick:async()=>{try{const pose=motion.lastPose;if(pose?.project!==project||!pose.result.asset_id)throw Error('Capture a pending pose in this project first.');const scene=Number(scenes.value);if(!Number.isSafeInteger(scene)||scene<=0)throw Error('Choose an authored destination scene.');const target=await motionCommand('storyboard.read',{scene});if(!current())return;if(!Number.isSafeInteger(target.page)||target.page<=0)throw Error('Scene has no authored storyboard page.');const coords=[x,y,width].map(f=>Number(f.input.value));if(coords.some(v=>!Number.isFinite(v))||coords[2]<=0)throw Error('Enter finite coordinates and positive width.');const params={asset:pose.result.asset_id,page:target.page,x:coords[0],y:coords[1],width:coords[2]};const result=await motionCommand('storyboard.place_asset',params);if(!current())return;if(result.ok!==true||result.placement?.asset!==params.asset||!result.draft)throw Error('Placement returned no draft confirmation.');pose.placement=result;poseResult.textContent=JSON.stringify(pose,null,2);poseStatus.textContent='Pose placed in storyboard draft · still pending approval. Review and Save in Project.';}catch(error){if(current())poseStatus.textContent=String(error);}}},'Place pose in storyboard draft');
 const loadScenes=el('button',{id:'authoring-pose-scenes',class:'ghost',onclick:async()=>{try{const toc=await motionCommand('storyboard.toc');if(!current())return;scenes.replaceChildren(...(toc.chapters||[]).filter(v=>Number.isSafeInteger(v.scene)&&v.scene>0).map(v=>el('option',{value:String(v.scene)},v.title||`Scene ${v.scene}`)));if(!scenes.options.length)throw Error('No authored storyboard scenes.');}catch(error){if(current())poseStatus.textContent=String(error);}}},'Load authored scenes');
 const poseNode=el('div',{class:'animation-authoring-fields'},reason('pose',caps.pose_export===true),reference,capture,poseStatus,posePreview,el('details',{},el('summary',{},'Place captured pose'),loadScenes,scenes,el('div',{class:'motion-coordinate-row'},x.node,y.node),width.node,place),poseResult);
 if(motion.lastPose?.project===project){poseResult.textContent=JSON.stringify(motion.lastPose,null,2);poseStatus.textContent='Captured pose · pending approval; project Save is explicit.';if(motion.lastPose.result.path){posePreview.src=convertFileSrc(motion.lastPose.result.path);posePreview.hidden=false;}}
 editControls.push(apply,reset,undo,actionApply,capture,...Object.values(values).flat().map(f=>f.input),label.input,duration.input,loop);for(const control of editControls)control.unavailable=control.disabled===true;
 return {status,part:partNode,action:actionNode,pose:poseNode};
}
