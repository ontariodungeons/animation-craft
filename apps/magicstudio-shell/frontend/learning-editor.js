import { Crepe } from '@milkdown/crepe';
import '@milkdown/crepe/theme/common/style.css';
import '@milkdown/crepe/theme/frame.css';
import { splitDocument, joinDocument, editableProblem, parseTask, validateTask } from './learning-markdown.mjs';

async function create(root,{markdown,onChange}) {
  const doc=splitDocument(markdown), problem=editableProblem(doc.body);
  root.addEventListener('click',event=>{const a=event.target.closest?.('a[href]');if(!a)return;event.preventDefault();event.stopImmediatePropagation();const href=a.getAttribute('href');if(/^(https?:\/\/|file:\/\/\/)/i.test(href)){if(typeof window.openPath==='function')window.openPath(href);}},true);
  if(problem) {root.textContent=problem;root.setAttribute('role','note');return {problem,markdown:()=>markdown,destroy:async()=>{}};}
  const crepe=new Crepe({root,defaultValue:doc.body,features:{[Crepe.Feature.AI]:false,[Crepe.Feature.Latex]:false,[Crepe.Feature.ImageBlock]:false,[Crepe.Feature.TopBar]:false}});
  let baseline,initializing=true;
  crepe.on(listener=>listener.markdownUpdated((_ctx,body)=>{if(!initializing)onChange(joinDocument(doc,body===baseline?doc.body:body));}));
  await crepe.create();baseline=crepe.getMarkdown();initializing=false;
  return {markdown:()=>{const body=crepe.getMarkdown();return joinDocument(doc,body===baseline?doc.body:body);},acceptSaved:(markdown,clean=true)=>{const saved=splitDocument(markdown);doc.prefix=saved.prefix;if(clean){doc.body=saved.body;baseline=crepe.getMarkdown();}},destroy:()=>crepe.destroy()};
}
window.LearningEditor={create,splitDocument,joinDocument,editableProblem,parseTask,validateTask};
