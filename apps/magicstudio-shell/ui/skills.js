// One bounded task file is the source for the human surface and CLI. No instruction execution.
const learning = { items:[], directory:null, selected:null, category:null, search:'', editor:null, epoch:0, dirty:false, conflict:null, removed:null };
const LEARNING_CATEGORIES = [['audio','Audio → lyrics'],['storyboard','Native storyboard'],['art','Art preparation'],['motion','Rig & motion'],['review','Review & release'],['correction','Correction loop']];
const lnode=(tag,cls,...children)=>el(tag,{class:cls},...children);
const lbutton=(id,label,action,title=label)=>el('button',{id,class:'ghost',onclick:action,title},label);
const lkey=(suffix='draft')=>`learning:${suffix}:${learning.directory}:${learning.selected?.id}`;
function learningStored(key){try{return JSON.parse(localStorage.getItem(key));}catch{return null;}}
function learningStore(key,value){try{if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,JSON.stringify(value));return true;}catch{learning.message='Draft could not be persisted locally. Keep this window open or copy it.';return false;}}
async function learningApi(id,params={}) {return invoke('engine_command',{id,params:{...params,...(learning.directory?{_expected_directory:learning.directory}:{})}});}
function learningError(e){learning.message=String(e);const n=$('learning-message');if(n)n.textContent=learning.message;}
async function renderSkills(){
  try{
    const result=await invoke('engine_command',{id:'skills.list',params:{}});
    if(learning.directory!==result.directory){await learning.editor?.destroy();learning.editor=null;learning.selected=null;learning.dirty=false;learning.conflict=null;learning.category=null;learning.directory=result.directory;}
    learning.items=result.skills||[];learning.limits=learning.items[0]?.limits||{};
    learningMount();learningRail();if(!learning.selected)learningHome();
  }catch(e){const root=$('view-skills');if(root&&!$('learning-message'))root.replaceChildren(lnode('p','why',String(e)));else learningError(e);}
}
function learningMount(){
  const root=$('view-skills');if(!root||$('learning-layout'))return;
  const search=el('input',{id:'learning-search',type:'search',placeholder:'Find a task / command',title:'Search task content'});
  search.addEventListener('input',()=>{learning.search=search.value;learningRail();if(!learning.selected)learningHome();});
  const rail=lnode('aside','learning-rail',lnode('h2','', 'Production manual'),search,
    lbutton('learning-cast','MOPPETS cast',castOpen,'Choose the Magic Studio cast and edit shared department AGENTS.md files'),
    lbutton('learning-overview','Production path',()=>{learning.category=null;learningHome();}),
    lnode('nav','learning-categories',...LEARNING_CATEGORIES.map(([id,title])=>lbutton(`learning-category-${id}`,title,()=>{learning.category=id;learningHome();}))),
    lnode('div','learning-task-list'),lnode('p','learning-location',learning.directory));
  rail.addEventListener('dragover',e=>e.preventDefault());rail.addEventListener('drop',e=>{e.preventDefault();const source=e.dataTransfer.getData('text/uri-list')||e.dataTransfer.getData('text/plain');if(source)learningImportSource(source.trim());});
  const main=lnode('main','learning-main',lnode('header','learning-bar',lnode('span','learning-breadcrumb','Learning / production path'),
    lnode('div','learning-actions',lbutton('learning-new','New task',learningNew),lbutton('learning-import','Import reference',learningImportClipboard,'Import a URL or existing package path from clipboard; also drop it on the list'),lbutton('learning-save','Save',()=>cast.active?castSave():learningSave()),lnode('span','learning-save-state'))),
    lnode('p','learning-message'),lnode('div','learning-surface'));
  main.querySelector('.learning-message').id='learning-message';main.querySelector('.learning-save-state').id='learning-save-state';
  root.replaceChildren(lnode('div','learning-layout',rail,main));root.firstChild.id='learning-layout';
}
function learningMatches(item){const q=learning.search.toLowerCase();return !q||[item.name,item.markdown,item.category].join(' ').toLowerCase().includes(q);}
function learningRail(){
  const list=$('view-skills')?.querySelector('.learning-task-list');if(!list)return;
  list.replaceChildren(...learning.items.filter(learningMatches).map(item=>el('button',{class:`learning-task ${learning.selected?.id===item.id?'active':''}`,'data-task':item.id,title:item.name,onclick:()=>learningOpen(item.id)},lnode('strong','',item.name||item.id),lnode('small','',`${item.category||'legacy reference'} · ${item.block?.word_count||0} words${item.blocked?' · blocked':''}`))));
}
async function learningHome(){
  castLeave();
  await learning.editor?.destroy();learning.editor=null;learning.selected=null;learning.dirty=false;learning.conflict=null;learning.epoch++;
  const surface=$('view-skills').querySelector('.learning-surface');surface.replaceChildren();surface.id='learning-home';
  $('learning-save').disabled=true;$('learning-save-state').textContent='';learningRail();
  const heading=learning.category?LEARNING_CATEGORIES.find(x=>x[0]===learning.category)?.[1]||learning.category:'One production process. Exact tasks.';
  surface.append(lnode('h1','',heading),lnode('p','learning-intro','Resolve the requested track, reuse its timing, then follow each bounded task. Missing authority is explicit.'));
  if(!learning.category&&!learning.search){
    const grid=lnode('div','learning-grid',...LEARNING_CATEGORIES.map(([id,title],i)=>el('button',{class:'learning-category-card',onclick:()=>{learning.category=id;learningHome();}},lnode('small','',String(i+1).padStart(2,'0')),lnode('h2','',title),lnode('span','',`${learning.items.filter(v=>v.category===id).length} bounded tasks`))));
    surface.append(grid,lnode('div','learning-actions',lbutton('learning-production-diagram','Production diagram',()=>learningDiagram('production')),lbutton('learning-correction-diagram','Correction diagram',()=>learningDiagram('correction'))));
  }
  const tasks=learning.items.filter(v=>learningMatches(v)&&(learning.category?v.category===learning.category:learning.search||['resolve-project','choose-audio','timing-skill','import-timing'].includes(v.id)));
  surface.append(lnode('div','learning-grid learning-task-grid',...tasks.map(v=>el('button',{class:'learning-summary','data-task':v.id,onclick:()=>learningOpen(v.id)},lnode('h3','',v.name),lnode('code','',v.block?.action?.ref||v.source||'Legacy reference'),lnode('p','',v.block?.output||'Original registry content preserved.'),v.blocked?lnode('p','learning-warning',v.blocked):null))));
  if(learning.removed)surface.append(lnode('div','learning-restore','Removed task preserved. ',lbutton('learning-restore','Undo removal',learningRestore)));
  learningMessage();
}
function learningMessage(){
  const policy=learning.limits?.max_words?`${learning.limits.max_words} words / ${learning.limits.max_steps} steps per task.`:'Word/step cap pending explicit confirmation.';
  $('learning-message').textContent=learning.message||policy;
}
async function learningOpen(id,options={}){
  castLeave();
  const epoch=++learning.epoch;
  try{
    const item=options.item||(await learningApi('skills.info',{id})).skill;if(epoch!==learning.epoch)return;
    await learning.editor?.destroy();if(epoch!==learning.epoch)return;
    learning.selected=item;learning.dirty=false;learning.conflict=null;learning.message='';
    const retained=learningStored(`learning:draft:${learning.directory}:${id}`);
    const draft=options.markdown??retained?.markdown;learning.draftRevision=options.baseRevision??retained?.revision??item.revision;
    const markdown=draft??item.markdown;learning.draft=markdown;learning.dirty=markdown!==item.markdown;
    const surface=$('view-skills').querySelector('.learning-surface');surface.id='learning-document';
    const mount=lnode('div','learning-writer');mount.id='learning-writer';
    const actions=lnode('div','learning-actions',lbutton('learning-back','← Task list',learningHome),lbutton('learning-copy','Copy exact block',()=>learningCopy()),lbutton('learning-remove','Remove',learningRemove));
    const conflict=lnode('section','learning-conflict');conflict.id='learning-conflict';conflict.hidden=true;
    const recover=lbutton('learning-recover-draft','Recover retained draft',()=>{const d=learningStored(lkey('recovery'));if(d)learningOpen(id,{item:learning.selected,markdown:d.markdown,baseRevision:d.revision});});recover.hidden=!learningStored(`learning:recovery:${learning.directory}:${id}`);
    const status=lnode('p','learning-validation');status.id='learning-validation';
    surface.replaceChildren(actions,recover,conflict,status,mount,lnode('div','learning-demo-host'),lnode('div','learning-related'));
    const api=await window.LearningEditor.create(mount,{markdown,onChange:md=>{if(learning.selected?.id!==id)return;learning.draft=md;learning.dirty=md!==learning.selected.markdown;learningStore(lkey(),learning.dirty?{markdown:md,revision:learning.draftRevision}:null);learningEditorState();learningDemo();}});
    if(epoch!==learning.epoch){await api.destroy();return;}learning.editor=api;learning.blocked=api.problem;
    $('learning-save').disabled=!!api.problem;learningEditorState();learningDemo();learningRail();
    if(learning.dirty && learning.draftRevision!==item.revision)learningShowConflict(item);
    const related=surface.querySelector('.learning-related');
    if(item.related?.length)related.append(lnode('h3','','Apply a technique only when this task needs it'),...item.related.map(ref=>lbutton(`learning-related-${ref}`,learning.items.find(v=>v.id===ref)?.name||ref,()=>learningOpen(ref))));
  }catch(e){learningError(e);}
}
function learningEditorState(){
  const item=learning.selected;if(!item)return;
  const block=window.LearningEditor.parseTask(learning.draft);
  const errors=item.kind==='task'?window.LearningEditor.validateTask(block,learning.limits):[];
  $('learning-validation').textContent=learning.blocked||errors.join(' · ')||`${block.word_count} words · ${block.step_count} steps · ${item.verification||'legacy record'}`;
  $('learning-save-state').textContent=learning.conflict?'Conflict · draft retained':learning.dirty?'Unsaved · draft retained':'Saved';
  $('learning-save').disabled=!!learning.blocked||!!learning.conflict||errors.length>0;
}
async function learningSave(){
  const item=learning.selected;if(!item||learning.blocked||learning.conflict)return;
  const directory=learning.directory,epoch=learning.epoch,key=lkey(),markdown=learning.editor.markdown();
  const current=()=>learning.directory===directory&&learning.epoch===epoch&&learning.selected?.id===item.id;
  try{
    const params={id:item.id,markdown,...(item.revision?{expected_revision:learning.draftRevision??item.revision}:{kind:'task',category:item.category||learning.category||'reference'})};
    const result=await learningApi(item.revision?'skills.edit':'skills.add',params);
    if(learningStored(key)?.markdown===markdown)learningStore(key,null);
    if(!current())return;
    const continued=learning.editor.markdown()!==markdown;
    learning.editor.acceptSaved?.(result.skill.markdown,!continued);
    learning.selected=result.skill;learning.draftRevision=result.skill.revision;learning.conflict=null;
    if(!continued){learning.draft=result.skill.markdown;learning.dirty=false;}else{learning.draft=learning.editor.markdown();learning.dirty=true;learningStore(key,{markdown:learning.draft,revision:learning.draftRevision});}
    learning.items=learning.items.filter(v=>v.id!==item.id).concat(result.skill);learningEditorState();learningRail();
  }catch(e){if(current()){await learningCheck();learningError(e);}}
}
function learningShowConflict(latest){
  const item=learning.selected;learning.conflict=latest;const panel=$('learning-conflict');if(!panel)return;panel.hidden=false;
  panel.replaceChildren(lnode('strong','','CLI/UI content changed. Both versions are retained.'),lnode('div','learning-comparison',lnode('pre','',learning.draft),lnode('pre','',latest.markdown)),lbutton('learning-use-latest','Use latest · retain my draft',()=>{learningStore(lkey('recovery'),{markdown:learning.draft,revision:learning.draftRevision});learningStore(lkey(),null);learningOpen(item.id,{item:latest});}));learningEditorState();
}
async function learningCheck(){
  const item=learning.selected;if(!item?.revision||learning.checking)return;learning.checking=true;
  try{
    const latest=(await learningApi('skills.info',{id:item.id})).skill;
    if(learning.selected?.id!==item.id||latest.revision===item.revision)return;
    if(!learning.dirty){if(latest.removed)await learningHome();else await learningOpen(item.id,{item:latest});return;}
    learningShowConflict(latest);
  }catch(e){learningError(e);}finally{learning.checking=false;}
}
async function learningRemove(){
  const item=learning.selected;if(!item?.revision)return;
  if(learning.dirty){learningError('Save or retain/reconcile this draft before removal.');return;}
  try{learning.removed=(await learningApi('skills.remove',{id:item.id,expected_revision:item.revision})).skill;learning.items=learning.items.filter(v=>v.id!==item.id);await learningHome();}catch(e){learningError(e);}
}
async function learningRestore(){try{const item=learning.removed;const r=await learningApi('skills.undo',{id:item.id,expected_revision:item.revision});learning.removed=null;learning.items.push(r.skill);learningRail();await learningOpen(item.id,{item:r.skill});}catch(e){learningError(e);}}
function learningNew(){
  const id=`task-${crypto.randomUUID().slice(0,8)}`;
  const markdown='# Untitled task\n\n## Input\n\n## Command\n\n## Output\n\n## Check\n';
  learningOpen(id,{item:{id,markdown,kind:'task',category:learning.category||'reference',verification:'draft'}});
}
async function learningImportClipboard(){try{const source=await navigator.clipboard.readText();if(!source.trim())throw Error('Copy a reference URL or existing skill/package path first, or drop it on the task list.');await learningImportSource(source.trim());}catch(e){learningError(e);}}
async function learningImportSource(source){try{const r=await learningApi('skills.import',{id:`ref-${crypto.randomUUID().slice(0,8)}`,source,category:learning.category||'reference'});learning.items.push(r.skill);await learningOpen(r.skill.id,{item:r.skill});}catch(e){learningError(e);}}
async function learningCopy(){try{const item=learning.selected;const markdown=learning.editor?.markdown()||learning.draft;await navigator.clipboard.writeText(JSON.stringify({id:item.id,category:item.category,...window.LearningEditor.parseTask(markdown),markdown},null,2));learning.message='Copied structured content; nothing executed.';learningMessage();}catch(e){learningError(e);}}
function learningDemo(){
  const item=learning.selected,host=$('view-skills').querySelector('.learning-demo-host');if(!host)return;host.replaceChildren();if(!item?.demo)return;
  const word=learning.draft.match(/## Demo word\s*\n([^\n]+)/)?.[1]?.trim().slice(0,12)||(item.demo==='matte'?'HOLD':'FLOW');
  const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('viewBox','0 0 680 200');svg.setAttribute('role','img');svg.setAttribute('aria-label','Original technique mechanism illustration; After Effects reference, Craft support unverified');
  const add=(tag,attrs,text)=>{const n=document.createElementNS(svg.namespaceURI,tag);for(const[k,v]of Object.entries(attrs))n.setAttribute(k,v);if(text)n.textContent=text;svg.append(n);return n;};
  if(item.demo==='trails')for(let i=0;i<3;i++){add('rect',{x:12+i*220,y:14,width:206,height:170,rx:8,fill:'#101722',stroke:'#34404f'});add('text',{x:26+i*220,y:40,fill:'#9cafc6','font-size':13},`t=${(i*.25).toFixed(2)}`);for(let j=0;j<3;j++)add('text',{x:28+i*220+i*12-j*5,y:82+j*29,fill:j?'#baa5eb':'#64d7c2',opacity:1-j*.25,'font-size':24},word);}
  if(item.demo==='matte'){add('text',{x:26,y:116,fill:'#64d7c2','font-size':88},word);['TEXT / MATTE','SOLID / ALPHA INVERTED','ANIMATED BACKING →'].forEach((v,i)=>{add('rect',{x:348+i*12,y:35+i*44,width:284,height:34,rx:4,fill:['#514467','#233041','#173332'][i]});add('text',{x:358+i*12,y:57+i*44,fill:'#e8ecf3','font-size':12},v);});}
  if(item.demo==='polar'){for(let i=0;i<5;i++)add('rect',{x:24,y:28+i*31,width:210,height:8,fill:i%2?'#baa5eb':'#64d7c2'});add('text',{x:280,y:112,fill:'#64d7c2','font-size':42},'→');for(const r of [82,62,42,22])add('circle',{cx:520,cy:102,r,fill:'none',stroke:'#baa5eb','stroke-width':2});}
  host.append(lnode('h3','','Mechanism reference'),svg,lnode('small','','Original illustration · AE recipe · Craft equivalents unverified. Edit descriptions and Demo word on the writing surface.'));
}
async function learningDiagram(kind){
  castLeave();
  try{
    learning.message='Generating Archify from these task records…';learningMessage();const result=await learningApi('skills.diagram',{kind,render:true});
    const surface=$('view-skills').querySelector('.learning-surface');await learning.editor?.destroy();learning.editor=null;learning.selected=null;
    const frame=el('iframe',{class:'learning-diagram',title:`Archify ${kind} workflow`,sandbox:'allow-scripts allow-downloads'});frame.srcdoc=result.html;
    surface.replaceChildren(lbutton('learning-back','← Task list',learningHome),lnode('p','learning-intro','Archify JSON is editable source. Editing a task updates the next generated view; no drag editor is implied.'),frame);learning.message='Archify generated from task IDs and transitions.';learningMessage();
  }catch(e){learningError(e);}
}
function setupSkillsAdd(){
  if(learning.timer)return;
  learning.timer=setInterval(()=>{if($('view-skills')?.classList.contains('active'))learningCheck();},1800);
  window.learningImportSource=learningImportSource;
}
