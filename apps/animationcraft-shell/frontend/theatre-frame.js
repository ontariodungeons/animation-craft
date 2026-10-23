// Local integration evaluation. Theatre Studio 0.7.0 is AGPL-3.0-only;
// release packaging requires a license review. No copied editor implementation.
import {getProject, types, onChange} from '@theatre/core';
import studio from '@theatre/studio';
import {theatreProjectId} from './project-id.mjs';

studio.initialize({usePersistentStorage:false});
try{
 studio.transaction(({stateEditors})=>{
  stateEditors.studio.historic.panelPositions.setPanelPosition({
   panelId:'sequenceEditor',
   position:{edges:{
    left:{from:'screenLeft',distance:0.18},
    right:{from:'screenRight',distance:0.01},
    top:{from:'screenTop',distance:0.02},
    bottom:{from:'screenBottom',distance:0.02}
   }}
  });
 });
}catch{}
let sheet, object, project, projectPath, projectId, stop, stopClock, stopPlaying, synchronizing=false, mirroredPosition, contextRevision=0;
let partStops=[];
const projects=new Map(), sheetObjects=new Map(), lastRotations=new Map();
const ROTATION_CONFIG={rotation:types.number(0,{range:[-45,45]})};
const getSheetObject=(projId,sh,partId)=>{
 const key=`${projId}:${sh.address?.sheetId||'default'}:${partId}`;
 let obj=sheetObjects.get(key);
 if(!obj){obj=sh.object(partId,ROTATION_CONFIG);sheetObjects.set(key,obj);}
 return obj;
};
const send=(type, detail={})=>parent.postMessage({magicTheatre:true,type,...detail},location.origin);
// Conservatively protect interactions without inspecting Theatre's private history or
// polling createContentOfSaveFile(), which itself mutates revision history in 0.7.0.
let interactionRevision=0,keyframeCount=null;
const pointers=new Set();
// Read-only keyframe count of the open part's rotation; reported when an interaction changes it.
const countKeyframes=()=>sheet?.sequence?.__experimental_getKeyframes?.(object?.props?.rotation)?.length??null;
function reportKeyframes(){const count=countKeyframes();if(count===null||count===keyframeCount)return;if(keyframeCount!==null)send('keyframes',{sheet:sheet.address.sheetId,part:object.address.objectKey,count,previous:keyframeCount});keyframeCount=count;}
for(const type of ['pointerdown','pointerup','pointercancel','input','keydown'])window.addEventListener(type,event=>{
 if(!projectPath)return;
 if(type==='pointerdown')pointers.add(event.pointerId);
 if(type==='pointerup'||type==='pointercancel')pointers.delete(event.pointerId);
 send('interaction',{project:projectPath,revision:++interactionRevision,active:pointers.size>0});
 if(!pointers.size)setTimeout(reportKeyframes,0);
},{capture:true,passive:true});
studio.onSelectionChange(selection=>send('selection',{objects:selection.map(item=>({sheet:item.address.sheetId,part:item.address.objectKey||null}))}));
window.addEventListener('message',async event=>{
 if(event.source!==parent||!event.data?.magicTheatre)return;
 try{
  const message=event.data;
  if(message.type==='context'){
   const revision=++contextRevision, id=await theatreProjectId(message.project);
   if(revision!==contextRevision)return;
   stop?.();stopClock?.();stopPlaying?.();for(const fn of partStops)fn?.();partStops=[];lastRotations.clear();
   project=projects.get(id);
   if(!project){project=getProject(id,message.state?{state:message.state}:undefined);projects.set(id,project);}
   projectId=id;projectPath=message.project;
   sheet=project.sheet(message.sheet||message.character);
   const parts=Array.isArray(message.parts)&&message.parts.length?message.parts:[message.part];
   const objects=new Map();
   for(const partId of parts){if(partId)objects.set(partId,getSheetObject(id,sheet,partId));}
   object=objects.get(message.part)||getSheetObject(id,sheet,message.part);
   await project.ready;
   if(revision!==contextRevision)return;
   studio.setSelection([object]);
   mirroredPosition=sheet.sequence.position;
   stop=object.onValuesChange(value=>{
    const prev=lastRotations.get(message.part);
    lastRotations.set(message.part,value.rotation);
    if(prev===undefined||Math.abs((value.rotation||0)-prev)>1e-4)send('values',{part:message.part,rotation:value.rotation,sync:synchronizing});
   });
   for(const [partId,partObj] of objects){
    if(partId===message.part)continue;
    partStops.push(partObj.onValuesChange(value=>{
     const prev=lastRotations.get(partId);
     lastRotations.set(partId,value.rotation);
     if(prev===undefined?Math.abs(value.rotation||0)>1e-4:Math.abs((value.rotation||0)-prev)>1e-4)send('values',{part:partId,rotation:value.rotation,sync:synchronizing});
    }));
   }
   stopClock=onChange(sheet.sequence.pointer.position,time=>{if(!synchronizing&&Math.abs(time-mirroredPosition)>1e-7)send('seek',{time});});
   stopPlaying=onChange(sheet.sequence.pointer.playing,playing=>send('playback',{playing}));
   keyframeCount=countKeyframes();
   const sequencedParts=[];
   for(const [partId,partObj] of objects){
    try{if(sheet.sequence.__experimental_getKeyframes(partObj.props.rotation).length>0)sequencedParts.push(partId);}catch{}
   }
   send('ready',{part:message.part,sequencedParts});
  }else if(message.type==='export'){
   if(!project||message.project!==projectPath)throw new Error('Theatre export project mismatch; reload the intended project');
   send('exported',{request:message.request,project:projectPath,state:studio.createContentOfSaveFile(projectId)});
  }else if(message.type==='seek'&&sheet){
   synchronizing=true;sheet.sequence.pause();mirroredPosition=message.time;sheet.sequence.position=message.time;synchronizing=false;
  }
 }catch(error){send('error',{error:String(error),request:event.data.request});}
});
send('loaded');
