// Local integration evaluation. Theatre Studio 0.7.0 is AGPL-3.0-only;
// release packaging requires a license review. No copied editor implementation.
import {getProject, types, onChange} from '@theatre/core';
import studio from '@theatre/studio';
import {theatreProjectId} from './project-id.mjs';

studio.initialize({usePersistentStorage:false});
let sheet, object, project, projectPath, projectId, stop, stopClock, stopPlaying, synchronizing=false, mirroredPosition, contextRevision=0;
const projects=new Map();
const send=(type, detail={})=>parent.postMessage({magicTheatre:true,type,...detail},location.origin);
studio.onSelectionChange(selection=>send('selection',{objects:selection.map(item=>({sheet:item.address.sheetId,part:item.address.objectKey||null}))}));
window.addEventListener('message',async event=>{
 if(event.source!==parent||!event.data?.magicTheatre)return;
 try{
  const message=event.data;
  if(message.type==='context'){
   const revision=++contextRevision, id=await theatreProjectId(message.project);
   if(revision!==contextRevision)return;
   stop?.();stopClock?.();stopPlaying?.();
   project=projects.get(id);
   if(!project){project=getProject(id,message.state?{state:message.state}:undefined);projects.set(id,project);}
   projectId=id;projectPath=message.project;
   sheet=project.sheet(message.character);
   object=sheet.object(message.part,{rotation:types.number(0,{range:[-45,45]})});
   await project.ready;
   if(revision!==contextRevision)return;
   studio.setSelection([object]);
   mirroredPosition=sheet.sequence.position;
   stop=object.onValuesChange(value=>send('values',{part:message.part,rotation:value.rotation}));
   stopClock=onChange(sheet.sequence.pointer.position,time=>{if(!synchronizing&&Math.abs(time-mirroredPosition)>1e-7)send('seek',{time});});
   stopPlaying=onChange(sheet.sequence.pointer.playing,playing=>send('playback',{playing}));
   send('ready',{part:message.part});
  }else if(message.type==='export'){
   if(!project||message.project!==projectPath)throw new Error('Theatre export project mismatch; reload the intended project');
   send('exported',{request:message.request,project:projectPath,state:studio.createContentOfSaveFile(projectId)});
  }else if(message.type==='seek'&&sheet){
   synchronizing=true;sheet.sequence.pause();mirroredPosition=message.time;sheet.sequence.position=message.time;synchronizing=false;
  }
 }catch(error){send('error',{error:String(error),request:event.data.request});}
});
send('loaded');
