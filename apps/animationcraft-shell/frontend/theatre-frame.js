// Local integration evaluation. Theatre Studio 0.7.0 is AGPL-3.0-only;
// release packaging requires a license review. No copied editor implementation.
import {getProject, types, onChange} from '@theatre/core';
import studio from '@theatre/studio';
import {theatreProjectId} from './project-id.mjs';

studio.initialize({usePersistentStorage:false});
let sheet, object, stop, stopClock, synchronizing=false, mirroredPosition, contextRevision=0;
const send=(type, detail={})=>parent.postMessage({magicTheatre:true,type,...detail},location.origin);
window.addEventListener('message',async event=>{
 if(event.source!==parent||!event.data?.magicTheatre)return;
 try{
  const message=event.data;
  if(message.type==='context'){
   const revision=++contextRevision, projectId=await theatreProjectId(message.project);
   if(revision!==contextRevision)return;
   stop?.();stopClock?.();
   const project=getProject(projectId);
   sheet=project.sheet(message.character);
   object=sheet.object(message.part,{rotation:types.number(0,{range:[-45,45]})});
   await project.ready;
   studio.setSelection([object]);
   mirroredPosition=sheet.sequence.position;
   stop=object.onValuesChange(value=>send('values',{part:message.part,rotation:value.rotation}));
   stopClock=onChange(sheet.sequence.pointer.position,time=>{if(!synchronizing&&Math.abs(time-mirroredPosition)>1e-7)send('seek',{time});});
   send('ready',{part:message.part});
  }else if(message.type==='seek'&&sheet){
   synchronizing=true;sheet.sequence.pause();mirroredPosition=message.time;sheet.sequence.position=message.time;synchronizing=false;
  }
 }catch(error){send('error',{error:String(error)});}
});
send('loaded');
