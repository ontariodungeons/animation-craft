// ---- Storyboard: title cards ------------------------------------------------------------

function renderStoryboard() {
  updateAppFooter();
  const sb = state.data?.storyboard;
  document.body.classList.toggle("movie-document",!!sb?.deck);
  const pages = sb?.previews || [];
  const chips = $("sb-chips");
  chips.replaceChildren();
  const cards = $("sb-cards");
  $("sb-empty")?.remove();

  for (const id of ["sb-prev", "sb-next", "sb-open-deck", "sb-save-version"]) $(id).disabled = !pages.length;
  if (!pages.length) {
    cards.replaceChildren();
    delete cards.dataset.document;
    $("sb-page").textContent = "Storyboard";
    cards.append(projectSetupCard());
    return;
  }
  state.page = Math.max(0, Math.min(state.page, pages.length - 1));
  const first = (sb.first_scene_page || 2) - 1;
  const nav=pages.map((_,i)=>[sb.page_titles?.[i]||sb.scene_titles?.[i-first]||`Page ${i+1}`,i]);
  state.pageTitles=Object.fromEntries(nav.map(([label,page])=>[page,label]));
  for (const [i, [label, page]] of nav.entries()) {
    const active = page === state.page;
    const isChapter = i < first;
    chips.append(el("button", { class: `chip${active ? " active" : ""}${isChapter ? " chapter" : " scene"}`, role: "tab", "aria-selected": String(active), title: label,
      onclick: () => showPage(page) }, label));
  }
  chips.querySelector(".active")?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  $("sb-page").textContent = `${state.page + 1} / ${pages.length}`;

  const wrap=cards;
  const key=JSON.stringify([state.project,pages,sb.layout,sb.page_size]);
  if(wrap.dataset.document!==key) {
    wrap.dataset.document=key;wrap.replaceChildren();wrap.classList.add("sb-page-wrap","document-pages");
    const size=storyboardPageSize();
    pages.forEach((path,i)=>{
      const page=el("div",{id:`sb-sheet-${i}`,class:"sb-sheet"});
      page.style.setProperty('--ar',`${size[0]}/${size[1]}`);
      page.append(el("img",{src:convertFileSrc(path),alt:`Storyboard page ${i+1}`,loading:"lazy"}));
      for(const [j,link] of (sb.layout?.links||[]).entries())if(link.page===i+1){
        const [x,y,r,b]=link.rect;
        const linkButton=el("button",{id:`sb-link-${i}-${j}`,class:"sb-link",title:`Go to page ${link.to_page}`,"aria-label":`Go to page ${link.to_page}`,onclick:()=>showPage(link.to_page-1)});
        linkButton.style.setProperty('--x',`${x/size[0]*100}%`);
        linkButton.style.setProperty('--y',`${y/size[1]*100}%`);
        linkButton.style.setProperty('--w',`${(r-x)/size[0]*100}%`);
        linkButton.style.setProperty('--h',`${(b-y)/size[1]*100}%`);
        page.append(linkButton);
      }
      page.addEventListener('dragover',e=>{e.preventDefault();page.classList.add('drop-target');e.dataTransfer.dropEffect='copy';});
      page.addEventListener('dragleave',()=>page.classList.remove('drop-target'));
      page.addEventListener('drop',async e=>{
        e.preventDefault();page.classList.remove('drop-target');
        const id=e.dataTransfer.getData('application/x-magicstudio-asset');if(!id)return;
        const r=page.getBoundingClientRect();
        try{await designCommand('storyboard.place_asset',{asset:id,page:i+1,x:(e.clientX-r.left)/r.width*size[0]-80,y:(e.clientY-r.top)/r.height*size[1]-80,width:160});}catch(error){toast(String(error));}
      });
      wrap.append(page);
    });
    wrap.onscroll=()=>{
      let current=0;const top=wrap.getBoundingClientRect().top;
      [...wrap.children].forEach((page,i)=>{if(page.getBoundingClientRect().top<=top+wrap.clientHeight*.35)current=i;});
      if(state.page===current)return;
      state.page=current;state.storyboardScene=(state.data?.storyboard_contents?.chapters||[]).find(scene=>scene.page===current+1)?.target || null;
      chips.querySelectorAll('[role=tab]').forEach((tab,i)=>{tab.classList.toggle('active',i===current);tab.setAttribute('aria-selected',String(i===current));});
      $("sb-page").textContent=`${current+1} / ${pages.length}`;renderBlocks();updateAppFooter();
    };
    $(`sb-sheet-${state.page}`)?.scrollIntoView({block:'start',behavior:'instant'});
  }

  renderStoryboardText();
  renderDesignPlacements();
}

function showPage(n) {
  const total = state.data?.storyboard?.previews?.length || 0;
  state.page = Math.max(0, Math.min(total - 1, n));
  state.storyboardScene=(state.data?.storyboard_contents?.chapters || []).find(scene=>scene.page===state.page+1)?.target || null;
  renderStoryboard();
  renderBlocks();
  $(`sb-sheet-${state.page}`)?.scrollIntoView({block:"start",behavior:"instant"});
}


function storyboardPageSize() { return state.data?.storyboard?.page_size || state.data?.storyboard?.layout?.page_size || [960,540]; }

function renderStoryboardText(){
 const size=storyboardPageSize();
 const kept=new Set();
 for(const field of state.data?.design?.text_fields||[]){
  const page=$(`sb-sheet-${field.page-1}`);if(!page)continue;
  const id=`sb-text-${field.page}-${field.id}`;kept.add(id);
  let node=$(id);
  if(!node){
   node=el('textarea',{id,class:'sb-text',title:`Edit ${field.field.replaceAll('_',' ')} · saved when you finish typing`,
    'aria-label':`Scene ${field.scene||'page '+field.page} ${field.field.replaceAll('_',' ')}`,'data-commands':'storyboard.edit',spellcheck:'true'});
   node.addEventListener('input',()=>{
    if(node.value===node.defaultValue)projectView.textDrafts.delete(id);
    else projectView.textDrafts.set(id,{page:field.page,field:field.field,value:node.value});
    updateAppFooter();
   });
   node.addEventListener('change',()=>saveStoryboard().catch(error=>toast(String(error))));
   page.append(node);
  }
  const draft=projectView.textDrafts.get(id);
  if(!draft){node.value=node.defaultValue=field.value || '';}
  else if(node.value!==draft.value){node.value=draft.value;}
  const [x,y,w,h]=field.rect;
  const put=(k,v)=>node.style.setProperty(k,v);
  put('--x',`${x/size[0]*100}%`);put('--y',`${y/size[1]*100}%`);put('--w',`${w/size[0]*100}%`);put('--h',`${(field.name?h:Math.max(h,50))/size[1]*100}%`);
  put('--font',`${(field.name?(field.font_size||12):(field.field==='lyrics'?27:21))/size[0]*100}cqw`);
  if(field.name){node.dataset.named='';put('--pad',`${2/size[0]*100}cqw`);put('--field-fill',field.fill);put('--field-ink',field.text_color);}
 }
 document.querySelectorAll('.sb-text').forEach(node=>{if(!kept.has(node.id))node.remove();});
}

function updateStoryboardSaveControls(){
 const disabled=projectView.pending>0 || !!state.data?.design?.error || (!state.data?.design?.dirty && !projectView.textDrafts.size);
 for(const id of ['sb-save-version','storyboard-save'])if($(id))$(id).disabled=disabled;
}

let storyboardSave=null;
function saveStoryboard(){
 if(storyboardSave)return storyboardSave;
 const project=state.project;
 storyboardSave=(async()=>{
  let result;
  do { result=await persistStoryboard(); }
  while(state.project===project && projectView.textDrafts.size);
  return result;
 })().finally(()=>{storyboardSave=null;updateStoryboardSaveControls();});
 return storyboardSave;
}

async function persistStoryboard(){
 const project=state.project;
 if(state.data?.design?.error)throw new Error(`Cannot save until the editable storyboard is readable: ${state.data.design.error}`);
 const edits=[...projectView.textDrafts.entries()];
 for(const [,edit] of edits)await designCommand('storyboard.edit',{...edit,save:false});
 const result=await designCommand('storyboard.save_version',state.data?.storyboard?.draft?{draft:state.data.storyboard.manifest}:{});
 if(state.project!==project)return result;
 for(const [id,edit] of edits)if(projectView.textDrafts.get(id)?.value===edit.value)projectView.textDrafts.delete(id);
 renderStoryboardText();renderBlocks();updateAppFooter();
 toast('Storyboard saved');return result;
}

let selectedPlacement=null;
function renderDesignPlacements(){
 const entries=state.data?.design?.placements||[];const size=storyboardPageSize();
 document.querySelectorAll('.design-placement,.design-remove').forEach(n=>n.remove());
 updateStoryboardSaveControls();
 $('sb-save-version').hidden=!(state.data?.storyboard?.pdf_fields||state.data?.storyboard?.deck);
 if(!entries.some(p=>p.id===selectedPlacement))selectedPlacement=entries.at(-1)?.id||null;

 for(const item of entries){
  const page=$(`sb-sheet-${item.page-1}`);if(!page)continue;
  const [x,y,w,h]=item.rect;
  const node=el('div',{id:item.id,class:'design-placement',title:'Drag to move · grab an edge to scale'});
  const place=(px,py,pw,ph)=>{node.style.setProperty('--x',`${100*px/size[0]}%`);node.style.setProperty('--y',`${100*py/size[1]}%`);if(pw!=null){node.style.setProperty('--w',`${100*pw/size[0]}%`);node.style.setProperty('--h',`${100*ph/size[1]}%`);}};
  place(x,y,w,h);
  node.append(el('img',{src:convertFileSrc(item.path),alt:'Placed design asset',draggable:'false'}));
  for(const [hx,hy] of [[-1,-1],[0,-1],[1,-1],[-1,0],[1,0],[-1,1],[0,1],[1,1]]){
   const handle=el('button',{class:'design-handle',title:'Drag to scale', 'aria-label':'Scale placed image','data-hx':hx,'data-hy':hy});
   handle.addEventListener('pointerdown',e=>{
    e.preventDefault();e.stopPropagation();selectedPlacement=item.id;handle.setPointerCapture(e.pointerId);
    const r=page.getBoundingClientRect(),start=[e.clientX,e.clientY];let rect=[x,y,w,h];
    handle.onpointermove=event=>{
     const dx=(event.clientX-start[0])*size[0]/r.width,dy=(event.clientY-start[1])*size[1]/r.height;
     const factor=hx===0?1+hy*dy/h:hy===0?1+hx*dx/w:1+(hx*dx/w+hy*dy/h)/2;
     const nw=Math.max(20,Math.min(size[0],w*factor)),nh=nw*h/w;
     rect=[hx<0?x+w-nw:hx===0?x+(w-nw)/2:x,hy<0?y+h-nh:hy===0?y+(h-nh)/2:y,nw,nh];
     place(rect[0],rect[1],nw,nh);
    };
    handle.onpointerup=async()=>{handle.onpointermove=null;handle.onpointerup=null;try{await designCommand('storyboard.place_asset',{asset:item.asset,page:item.page,placement:item.id,x:rect[0],y:rect[1],width:rect[2]});}catch(error){toast(String(error));}};
   });node.append(handle);
  }
  node.addEventListener('pointerdown',e=>{
   selectedPlacement=item.id;
   e.preventDefault();node.setPointerCapture(e.pointerId);const r=page.getBoundingClientRect(),start=[e.clientX,e.clientY];let nx=x,ny=y;
   node.onpointermove=event=>{nx=x+(event.clientX-start[0])*size[0]/r.width;ny=y+(event.clientY-start[1])*size[1]/r.height;place(nx,ny);};
   node.onpointerup=async()=>{node.onpointermove=null;node.onpointerup=null;try{await designCommand('storyboard.place_asset',{asset:item.asset,page:item.page,x:nx,y:ny,width:w,placement:item.id});}catch(error){toast(String(error));}};
  });const remove=el('button',{class:'design-remove',title:'Remove this placement','aria-label':'Remove this placement',onclick:()=>designCommand('storyboard.remove_placement',{placement:item.id}).catch(e=>toast(String(e)))},'×');
  remove.style.setProperty('--x',`${100*(x+w)/size[0]}%`);remove.style.setProperty('--y',`${100*y/size[1]}%`);page.append(node,remove);
 }
}
$('sb-save-version').addEventListener('click',()=>saveStoryboard().catch(error=>toast(String(error))));

$("sb-prev").addEventListener("click", () => showPage(state.page - 1));
$("sb-next").addEventListener("click", () => showPage(state.page + 1));
$("sb-open-deck").addEventListener("click", () => state.data?.storyboard?.deck && openPath(state.data.storyboard.deck));
window.addEventListener("keydown", (e) => {
  if (state.view !== "project" || e.target.closest("input, textarea")) return;
  if (e.key === "ArrowRight") showPage(state.page + 1);
  if (e.key === "ArrowLeft") showPage(state.page - 1);
});
