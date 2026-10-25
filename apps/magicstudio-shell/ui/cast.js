// Department AGENTS.md files are the shared source for this callboard and mc.
const cast = { active:false, items:[], drafts:new Map(), department:'creative', filter:'all', search:'', busy:false };
const CAST_STATES = {candidate:'Audition',cast:'Cast',reserve:'Reserve',wings:'In the wings'};
const cnode=(tag,cls,...children)=>el(tag,{class:cls},...children);
const cbutton=(label,action)=>el('button',{class:'ghost',title:label,'aria-label':label,onclick:action},label);
const castKey=id=>`cast:${cast.workspace}:${id}`;
const castApi=(id,params={})=>invoke('engine_command',{id,params});
const castItem=()=>cast.items.find(v=>v.document.department.id===cast.department);
function castDraft(item=castItem()) {
  const id=item.document.department.id;
  if(!cast.drafts.has(id)) {
    const retained=learningStored(castKey(id));
    cast.drafts.set(id,retained?.document?.department?.id===id?retained:{document:structuredClone(item.document),revision:item.revision});
  }
  return cast.drafts.get(id);
}
function castChanged() {
  const draft=castDraft(),stored=learningStore(castKey(cast.department),draft);
  castStatus(stored===false?learning.message:undefined);
}
function castStatus(message) {
  if(!cast.active)return;
  const item=castItem(),draft=castDraft(),dirty=JSON.stringify(draft.document)!==JSON.stringify(item.document);
  $('learning-save-state').textContent=cast.busy?'Saving…':dirty?'Unsaved · draft retained':'Saved';
  $('learning-save').disabled=cast.busy||!dirty;
  $('learning-message').textContent=message||`${item.path} · ${draft.revision!==item.revision?'File changed; draft retained. Compare before saving.':'Shared rules for this department.'}`;
}
function castLeave() {
  cast.active=false;
  for(const id of ['learning-new','learning-import'])if($(id))$(id).hidden=false;
  if($('learning-save'))$('learning-save').textContent='Save';
  const crumb=$('view-skills')?.querySelector('.learning-breadcrumb');if(crumb)crumb.textContent='Learning / production path';
}
async function castOpen() {
  try {
    const result=await castApi('crew.list');
    await learning.editor?.destroy();learning.editor=null;learning.selected=null;learning.epoch++;
    if(cast.workspace!==result.workspace)cast.drafts.clear();
    else for(const item of cast.items){
      const id=item.document.department.id,draft=cast.drafts.get(id);
      if(draft&&JSON.stringify(draft.document)===JSON.stringify(item.document))cast.drafts.delete(id);
    }
    cast.workspace=result.workspace;cast.items=result.departments;cast.active=true;
    for(const id of ['learning-new','learning-import'])$(id).hidden=true;
    $('learning-save').textContent='Save AGENTS.md';
    $('view-skills').querySelector('.learning-breadcrumb').textContent='Learning / Magic Studio cast';
    castRender();
  }catch(e){learningError(e);}
}
function castField(label,node) {return cnode('label','cast-field',cnode('span','',label),node);}
function castSelect(label,options,value,change) {
  const select=el('select',{'aria-label':label,title:label});
  select.append(...options.map(([id,name])=>el('option',{value:id},name)));select.value=value??'';
  select.addEventListener('change',()=>change(select.value));return select;
}
function castInput(label,value,change,multiline=false) {
  const input=el(multiline?'textarea':'input',{'aria-label':label,title:label,...(!multiline?{type:'text'}:{rows:4})});
  input.value=input.defaultValue=value||'';input.addEventListener('input',()=>change(input.value));return input;
}
function castCard(candidate,department) {
  const update=(key,value)=>{candidate[key]=value;castChanged();};
  const card=cnode('details','cast-candidate');card.dataset.candidate=candidate.id;
  const heading=cnode('strong','',candidate.name),story=cnode('p','',candidate.personality),badge=cnode('small','cast-state',CAST_STATES[candidate.status]);
  card.append(cnode('summary','',cnode('div','cast-name',heading,badge),cnode('small','',candidate.role),story,
    ...(candidate.lead_suggestion?[cnode('small','cast-suggestion','Lead idea')]:[])));
  const status=castSelect(`Casting for ${candidate.name}`,Object.entries(CAST_STATES),candidate.status,value=>{
    if(department.lead===candidate.id&&value!=='cast')department.lead=null;
    update('status',value);badge.textContent=CAST_STATES[value];castLeadOptions();
  });
  const fields=cnode('div','cast-fields',
    castField('Name',castInput(`Name for ${candidate.id}`,candidate.name,value=>{update('name',value);heading.textContent=value;castLeadOptions();})),
    castField('Casting',status),
    castField('Role',castSelect(`Role for ${candidate.id}`,department.roles.map(v=>[v,v]),candidate.role,value=>update('role',value))),
    castField('Model',castInput(`Model for ${candidate.id}`,candidate.model,value=>update('model',value.trim()||null))),
    castField('Capabilities · comma separated',castInput(`Capabilities for ${candidate.id}`,candidate.capabilities.join(', '),value=>update('capabilities',value.split(',').map(v=>v.trim()).filter(Boolean)))),
    castField('Personality',castInput(`Personality for ${candidate.id}`,candidate.personality,value=>{update('personality',value);story.textContent=value;},true)));
  fields.append(cnode('small','cast-note','Model and capabilities are choices for this profile. Session use appears only when measured.'));
  card.append(fields);return card;
}
function castLeadOptions() {
  const select=$('cast-lead');if(!select)return;
  const d=castDraft().document.department;
  select.replaceChildren(el('option',{value:''},'Choose a lead'),...d.candidates.filter(v=>v.status==='cast').map(v=>el('option',{value:v.id},v.name)));
  select.value=d.lead||'';
}
function castRender() {
  const surface=$('view-skills').querySelector('.learning-surface');surface.id='cast-page';
  const item=castItem(),doc=castDraft().document,d=doc.department;
  const total=cast.items.reduce((n,v)=>n+castDraft(v).document.department.candidates.length,0);
  const hero=cnode('header','cast-hero',cnode('small','group-label','AI–Human MOPPETS'),cnode('h1','','Magic Studio cast'),
    cnode('p','',`${total} hopefuls. Five departments. One human with the final say.`));
  const nav=cnode('nav','cast-departments',...cast.items.map(v=>{
    const dep=v.document.department,button=cbutton(dep.title,()=>{cast.department=dep.id;castRender();});
    button.setAttribute('aria-pressed',String(dep.id===d.id));return button;
  }));
  nav.setAttribute('aria-label','Cast departments');
  const lead=d.id==='review'?cnode('p','cast-human','Jesse · human lead. Summon an AI colleague when useful.'):
    castField('Department lead',castSelect('Department lead',[],d.lead,value=>{d.lead=value||null;castChanged();}));
  if(d.id!=='review')lead.querySelector('select').id='cast-lead';
  const suggestions=d.candidates.filter(v=>v.lead_suggestion);
  const intro=cnode('section','cast-intro',cnode('h2','',d.title),cnode('p','',d.purpose),lead,
    cnode('p','cast-note',`${d.id==='review'?'AI colleague ideas':'Lead ideas'}: ${suggestions.map(v=>v.name).join(' · ')}. Cast someone to make them available as a lead.`));
  const search=castInput('Find a cast candidate',cast.search,value=>{cast.search=value;castCards();});search.type='search';search.placeholder='Find a name, role or personality';search.id='cast-search';
  const filter=castSelect('Casting filter',[['all','Everyone'],...Object.entries(CAST_STATES)],cast.filter,value=>{cast.filter=value;castCards();});filter.id='cast-filter';
  const tools=cnode('div','cast-tools',search,filter);
  const grid=cnode('div','cast-grid');grid.id='cast-candidates';
  const roles=castInput('Allowed department roles',d.roles.join(', '),value=>{d.roles=value.split(',').map(v=>v.trim()).filter(Boolean);castChanged();});
  roles.addEventListener('change',castCards);
  const rules=cnode('details','cast-rules',cnode('summary','','Shared department instructions'),
    cnode('p','cast-note','These instructions and the cast profiles save to this department’s AGENTS.md.'),
    castField('Purpose',castInput('Department purpose',d.purpose,value=>{d.purpose=value;castChanged();},true)),
    castField('Allowed roles · comma separated',roles),
    castField('Skills · comma separated',castInput('Department skills',d.skills.join(', '),value=>{d.skills=value.split(',').map(v=>v.trim()).filter(Boolean);castChanged();})),
    castField('Commands · comma separated',castInput('Department commands',d.commands.join(', '),value=>{d.commands=value.split(',').map(v=>v.trim()).filter(Boolean);castChanged();})),
    castInput('Shared department instructions',doc.instructions,value=>{doc.instructions=value;castChanged();},true),
    cnode('p','cast-note','Profiles must use one of the allowed roles. Discover command support with mc commands.'));
  const saved=cnode('details','cast-saved',cnode('summary','','Read saved AGENTS.md'),cnode('pre','',item.markdown));saved.id='cast-saved-file';
  const reload=cbutton('Load saved version · retain this draft',()=>{
    const draft=castDraft();learningStore(`${castKey(d.id)}:recovery`,draft);cast.drafts.delete(d.id);learningStore(castKey(d.id),null);castRender();
  });
  const recover=cbutton('Recover previous draft',()=>{const previous=learningStored(`${castKey(d.id)}:recovery`);if(previous){cast.drafts.set(d.id,previous);castChanged();castRender();}});
  recover.hidden=!learningStored(`${castKey(d.id)}:recovery`);
  surface.replaceChildren(hero,nav,intro,tools,grid,rules,saved,cnode('div','learning-actions',reload,recover));
  castLeadOptions();castCards();castStatus();
}
function castCards() {
  const d=castDraft().document.department,q=cast.search.toLowerCase();
  const candidates=d.candidates.filter(v=>(cast.filter==='all'||v.status===cast.filter)&&[v.name,v.role,v.personality].join(' ').toLowerCase().includes(q));
  $('cast-candidates').replaceChildren(...candidates.map(v=>castCard(v,d)));
  if(!candidates.length)$('cast-candidates').append(cnode('p','cast-note','No candidates match this view.'));
}
async function castSave() {
  if(cast.busy)return;
  const item=castItem(),draft=castDraft(),id=cast.department,workspace=cast.workspace;
  const document=structuredClone(draft.document);cast.busy=true;castStatus();
  try {
    const saved=await castApi('crew.save',{workspace,department:id,expected_revision:draft.revision,document});
    if(cast.workspace!==workspace)return;
    cast.items[cast.items.indexOf(item)]=saved.department;
    const current=cast.drafts.get(id);
    if(current===draft){current.revision=saved.department.revision;if(JSON.stringify(current.document)===JSON.stringify(document))learningStore(castKey(id),null);else learningStore(castKey(id),current);}
    if(cast.active&&cast.department===id){if(JSON.stringify(current.document)===JSON.stringify(document))$('cast-page')?.querySelectorAll('input,textarea').forEach(n=>n.defaultValue=n.value);$('cast-saved-file').querySelector('pre').textContent=saved.department.markdown;castStatus(`Saved ${saved.department.path}`);}
  }catch(e){
    if(cast.workspace===workspace&&cast.active&&cast.department===id){
      try{const latest=await castApi('crew.list',{workspace});if(cast.workspace===workspace)cast.items=latest.departments;}catch{}
      castStatus(`${e} · Your draft is retained.`);
    }
  }finally{cast.busy=false;if(cast.active)castStatus($('learning-message').textContent);}
}
