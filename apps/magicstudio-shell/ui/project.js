// One owner for Project commands, readiness and page selection.
class ProjectView {
  constructor(state) {
    this.state = state;
    this.pending = 0;
    this.error = null;
    this.queue = Promise.resolve();
    this.textDrafts = new Map();
  }
  stage(id) { return this.state.data?.status?.stages?.find(stage=>stage.id===id) || {inputs:[],outputs:[]}; }
  prerequisite(id) { return this.state.data?.monitoring?.prerequisites?.find(item=>item.id===id) || {state:'unknown'}; }
  scene() {
    const scenes=this.state.data?.storyboard_contents?.chapters || [];
    return scenes.find(scene=>scene.page===this.state.page+1) || scenes.find(scene=>scene.target===this.state.storyboardScene) || scenes[0];
  }
  render() { renderTarget(); renderBlocks(); renderStoryboard(); }
  command(id,params={}) {
    const project=this.state.project;
    const run=async()=>{
      if(!project || this.state.project!==project)throw new Error('The open project changed; command was not submitted');
      this.pending++;this.error=null;updateAppFooter();
      try {
        const result=await invoke('project_command',{project,id,params});
        if(this.state.project!==project)throw new Error('The open project changed; its previous command result was not applied');
        this.state.data=result;this.render();
        if(result?._ui_engine_revision)await acFeedback.acknowledge(result._ui_engine_revision,project);
        return result;
      } catch(error) {
        if(this.state.project===project){this.error=String(error.message||error);renderBlocks();}
        throw error;
      } finally { this.pending--;updateAppFooter(); }
    };
    const result=this.queue.then(run);
    this.queue=result.catch(()=>{});
    return result;
  }
}
const projectView=new ProjectView(state);
function designCommand(id,params={}) { return projectView.command(id,params); }

// A block is a shared UI box: it has a stable id, and its header row is a
// slot on the rail so a companion can enter the block. The header is the
// stable surface -- the body expands and collapses, so nothing there can
// hold a position.
//
// The body is wrapped, never rewritten: the same DOM arrives, and a block
// that does not care about companions inherits the contract for free.
function block(id, icon, title, meta, done, body, blockState, action) {
  const open = state.open === id;
  const dot = window.companionBoxes.dot(done ? "ready" : "needs-attention", "", id);
  dot.title = done ? "Ready" : "Needs work";
  const head = el("button", { class: "panel-head", id: `block-${id}`, "aria-label":title, "aria-expanded": String(open), title: `${title}${meta ? ` · ${meta}` : ""}`,
      onclick: () => {
        const narrow = getComputedStyle(document.documentElement).getPropertyValue("--workspace-narrow").trim() === "1";
        if (narrow && agent.open) { agentToggle(false); state.open = id; } else state.open = open ? null : id;
        renderBlocks();
      } },
    dot,
    el("span", { class: "ico", html: icon }),
    el("span", { class: "title" }, title),
    el("span", { class: "meta" }, meta || ""),
    el("span", { class: "chev", html: ICONS.chev }));
  // The header row carries the slot. It sits on the block's own edge, outside
  // the clickable head button, so a companion never covers a control.
  // Readiness informs a prerequisite status. An empty block is not an error.
  if (window.companionBoxes) window.companionBoxes.readiness(id, { state: blockState, reason: done ? "Ready" : "Needs work" });
  return el("section", { box: id, class: `block${open ? " open" : ""}`, "data-state": blockState, "data-block": id, "data-box-id": id },
    action ? el("div", { class: "block-headrow" }, head, action) : head,
    el("div", { class: "block-body" }, el("div", {}, el("div", { class: "block-inner" }, body))));
}


function projectSetupCard(creating=false) {
  const preview=el('div',{class:'project-setup-preview','aria-hidden':'true'},
    el('div',{class:'project-setup-sheet'},el('span',{class:'project-setup-art',html:ICONS.animate}),
      el('span',{class:'project-setup-line'}),el('div',{class:'project-setup-fields'},...Array.from({length:4},()=>el('span')))));
  const steps=el('ol',{class:'project-setup-steps','aria-label':'Project workflow'},
    ...[['song','Recording'],['lyrics','Lyrics'],['animate','Storyboard']].map(([icon,label])=>
      el('li',{},el('span',{class:'ico',html:ICONS[icon],'aria-hidden':'true'}),el('span',{},label))));
  return el('div',{id:'sb-empty',class:'panel-empty project-setup'},preview,
    el('strong',{},creating?'Start with your song':'Your storyboard'),
    el('p',{},creating?'Add a recording in the Song panel.':'One editable DeckCraft template.'),steps,
    creating?null:el('button',{id:'sb-create-storyboard',class:'start','data-commands':'storyboard.init',
      onclick:()=>designCommand('storyboard.init',{empty:true}).catch(error=>toast(String(error)))},'Create storyboard'));
}

function renderBlocks() {
  updateAppFooter();
  updateStoryboardAccess();
  const s = state.data?.status;
  const box = $("blocks");
  box.replaceChildren();
  if (!s) { if (state.creating) renderNewProjectBlocks(box); return; }
  const receipt=state.projectReceipt;
  if(receipt?.project===state.project && (receipt.problems?.length || receipt.warnings?.length)){
    box.append(el('details',{class:'project-notice',id:'project-intake-problems'},
      el('summary',{},el('span',{class:'project-notice-dot','aria-hidden':'true'}),
        el('strong',{},receipt.intake_ready===false?'Setup needs attention':'Project notices'),
        el('span',{class:'ico',html:ICONS.chev,'aria-hidden':'true'})),
      el('div',{class:'project-notice-body'},el('p',{class:'project-path'},receipt.project),
        ...(receipt.problems||[]).map(problem=>el('p',{},String(problem))),
        ...(receipt.warnings||[]).map(warning=>el('p',{},warning.error||warning.effect||warning.code||String(warning))))));
  }
  if(projectView.error)box.append(el('p',{class:'why',id:'project-command-error',role:'alert'},projectView.error));
  if(state.data?.design?.error)box.append(el('div',{class:'why',id:'storyboard-read-error',role:'alert'},
    el('p',{},`Cannot read the editable storyboard: ${state.data.design.error}`),
    el('button',{id:'storyboard-retry',class:'ghost',
      onclick:()=>designCommand('storyboard.design').catch(error=>toast(String(error)))},'Retry storyboard')));
  // Collapse/expand toggle for the blocks rail
  box.append(el("button", {
    class: `icon rail-toggle${state.blocksCollapsed ? "" : " open"}`,
    title: state.blocksCollapsed ? "Expand blocks rail" : "Collapse blocks rail",
    "aria-label": state.blocksCollapsed ? "Expand blocks rail" : "Collapse blocks rail",
    onclick: () => { state.blocksCollapsed = !state.blocksCollapsed; box.classList.toggle("collapsed", state.blocksCollapsed); renderBlocks(); }
  }, el("span", { html: state.blocksCollapsed ? '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 9l6 6 6-6"/><path d="M18 3H6c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM6 13l6-6-6-6zM12 15l-6 6 6 6z"/></svg>' : '<svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 9l6 6 6-6"/><path d="M18 3H6c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM6 13l6-6-6-6zM12 15l-6 6 6 6z"/></svg>' })));
  const stage = id => projectView.stage(id);
  const song = stage("song"), transcript = stage("transcript"), animate = stage("animate");
  const audio = song.inputs?.[0];
  const liner = (song.inputs || []).slice(1);

  const observed=id=>projectView.prerequisite(id);
  const ready=id=>observed(id)?.state==='ready';
  // Song holds its lyrics and transcription; each keeps its own readiness light and state.
  const songState = observed("song")?.state || "unknown";
  const lyricsState = observed("lyrics")?.state || "unknown";
  const outputs = (transcript.outputs || []).filter((c) => c.exists !== false && c.path !== audio?.path);
  const transcriptState = observed("transcript")?.state || "unknown";
  const doc = transcript.document;
  box.append(block("song", ICONS.song, "Song", audio?.name, ready("song"),
    [audio ? fileButton(audio) : null,
     el('button',{id:'song-upload','data-commands':'workshop.set_audio',onclick:async()=>{
       const path=await dialog.open({title:'Choose song audio',filters:[{name:'Audio',extensions:['wav','mp3','flac','m4a','aac','ogg']}]});
       if(path)try{await designCommand('workshop.set_audio',{path});toast('Audio link saved; review timing and storyboard for this recording.');}catch(error){toast(String(error));}
     }},audio?'Change audio…':'Upload audio…'),
     songSection("lyrics", "Lyrics & notes", state.lyrics?.source === "canonical" ? `${state.lyrics.line_count} lines · ${state.lyrics.sections.length} sections` : doc?.words?.length ? "from transcript" : "",
       ready("lyrics"), lyricsState, lyricsBody(state.lyrics, doc, liner, audio?.path)),
     songSection("transcript", "Transcription", doc?.words?.length ? transcriptSummary(doc, ready("transcript")) : "", ready("transcript"), transcriptState,
       [doc?.words?.length ? el("button", { id: "song-open-transcript", onclick: () => lyricReaderOpen(state.lyrics, doc, audio?.path, true) }, "Open transcript") : null,
        el('button',{id:'song-import-transcript','data-commands':'workshop.import_transcript',onclick:importProjectTranscript},'Import timed lyrics…'),
        !doc?.words?.length && audio ? el('button',{id:'song-transcribe','data-commands':'audio.analyze',onclick:()=>designCommand('audio.analyze',{analyze_beats:false}).catch(error=>toast(String(error)))},'Transcribe this recording') : null,
        el("details", { class: "song-details" }, el("summary", {}, "Details"), el("div", { class: "why" }, transcript.why || ""), ...outputs.map(fileButton))])],
    songState, audio ? songPlayButton(audio.path) : null));

  const template=state.data?.storyboard_template?.path||state.data?.storyboard?.template_file;
  const sb = state.data?.storyboard;
  const content = state.data?.design?.readiness;
  const missing = content?.missing_fields || [];
  const openPage = page => showPage(page - 1);
  const contents = state.data?.storyboard_contents;
  const scenes = contents?.chapters || [];
  const scene = projectView.scene();
  const scenePicker = el('select',{id:'storyboard-scene','aria-label':'Storyboard scene',onchange:event=>{state.storyboardScene=event.target.value; const selected=scenes.find(s=>s.target===state.storyboardScene); if(selected)showPage(selected.page-1); renderBlocks();}},
    ...scenes.map(s=>el('option',{value:s.target,selected:s.target===scene?.target},`${s.scene} · ${s.title}`)));
  const wordPicker = el('select',{id:'storyboard-word','aria-label':'Recorded word at the scene cut'},
    ...scenes.filter(s=>Math.abs(s.scene-(scene?.scene || 1))<=1).flatMap(s=>s.words || []).map(w=>el('option',{value:w.id,selected:w.id===scene?.words?.[1]?.id},`${Number(w.start).toFixed(2)} · ${w.text}`)));
  const sceneAction = (id,label,command,params,disabled=false) => el('button',{id,disabled:disabled || !scene,'data-commands':command,onclick:async()=>{
    try{const result=await designCommand(command,{scene:scene.target,expected_revision:contents.revision,...params(),word:wordPicker.value});
      if(result.previous_version)state.storyboardUndo=result.previous_version;
      toast('Storyboard saved · lyric timing unchanged');renderBlocks();
    }catch(error){toast(String(error));}
  }},label);
  box.append(block('storyboard',ICONS.animate,'Storyboard',content?.state==='ready'?'Content filled':'Needs work',ready('storyboard'),[
    el('div',{class:'field-row'},
      !sb?.deck ? el('button',{id:'storyboard-create',class:'ghost','data-commands':'storyboard.init',onclick:()=>designCommand('storyboard.init',{empty:true}).catch(error=>toast(String(error)))},'Create storyboard') : null,
      el('button',{id:'storyboard-template-open',class:'icon',disabled:!template,'aria-label':'Edit storyboard template',title:template||'Magic Studio storyboard template',html:$('sb-open-deck').innerHTML,onclick:()=>invoke('open_path',{path:template})})),
    sb?.deck ? el('p',{class:'why',role:'status'},content?.reason || 'Storyboard needs review') : null,
    el('button',{id:'storyboard-discuss','data-drawer-toggle':'sproket',title:'Open Sproket to discuss the missing storyboard details',onclick:async()=>{
      const input=$('agent-input');
      if(!input.value.trim())input.value='Help me develop this storyboard. Read center.status creative_followup and the existing project assets. Ask me about the missing creative details before proposing placements or generating artwork.';
      input.focus();
    }},'Discuss with Sproket'),
    el('button',{id:'storyboard-check','data-commands':'storyboard.design',onclick:()=>designCommand('storyboard.design').catch(error=>toast(String(error)))},'Check storyboard details'),
    sb?.exists && content?.state!=='ready' ? el('button',{id:'storyboard-draft-override','data-commands':'workshop.approve_draft',title:'Allow Animation and Lyrics for this draft while keeping missing details visible',onclick:()=>designCommand('workshop.approve_draft',{approved:!state.data?.design?.progression?.draft_override,allow_incomplete:true}).catch(error=>toast(String(error)))},state.data?.design?.progression?.draft_override?'Turn off draft access':'Proceed with this draft') : null,
    sb?.deck && sb.empty_template && ready('transcript') ? el('button',{id:'storyboard-create-scenes','data-commands':'storyboard.init',onclick:()=>designCommand('storyboard.init').catch(error=>toast(String(error)))},'Create scene pages from timing') : null,
    sb?.previews?.length ? el('button',{id:'storyboard-overview',onclick:()=>openPage(1)},sb.page_titles?.[0] || 'First page') : null,
    el('p',{class:'why'},`${sb?.previews?.length || 0} pages · ${(sb?.page_size || []).join(' × ')} · Edit text on the page; drag assets onto it.`),
    ...missing.map(item=>el('button',{'aria-label':`Edit page ${item.page} ${item.field}`,onclick:()=>openPage(item.page)},`Page ${item.page} · ${item.field}`)),
    ...(scenes.length ? [el('p',{class:'why'},`${scenes.length} scenes · Timing from this project’s recorded words.`),scenePicker,wordPicker,
    sceneAction('storyboard-add','Add scene before word','storyboard.create',()=>({}),(scene?.words?.length || 0)<2),
    sceneAction('storyboard-trim-start','Start scene at word','storyboard.update',()=>({edge:'start'}),scene?.scene===1),
    sceneAction('storyboard-trim-end','End scene at word','storyboard.update',()=>({edge:'end'}),scene?.scene===scenes.length),
    sceneAction('storyboard-delete','Combine with previous','storyboard.delete',()=>({into:'previous'}),scene?.scene===1),
    sceneAction('storyboard-delete-next','Combine with next','storyboard.delete',()=>({into:'next'}),scene?.scene===scenes.length),
    el('button',{id:'storyboard-undo',disabled:!state.storyboardUndo,'data-commands':'storyboard.update',onclick:async()=>{try{await designCommand('storyboard.update',{restore:state.storyboardUndo,expected_revision:contents.revision});state.storyboardUndo=null;renderBlocks();toast('Previous storyboard restored');}catch(error){toast(String(error));}}},'Undo scene change')] : []),
    sb?.deck ? el('button',{id:'storyboard-save',disabled:!state.data?.design?.dirty && !projectView.textDrafts.size,'data-commands':'storyboard.save_version',onclick:()=>saveStoryboard().catch(error=>toast(String(error)))},'Save new version') : null
  ],observed('storyboard')?.state || 'unknown'));

  const assets=state.data?.assets?.assets||[];
  const assetsState = observed("assets")?.state || "unknown";
  const assetFiles = (s.asset_files||[]);
  const assetsBody=[el('button',{id:'asset-import',class:'start',onclick:importProjectAsset},'Add asset'),projectAssetGrid(assets,{prefix:'asset'}),
    el('p',{class:'why'},state.data?.design?.progression?.allowed?'Click an asset to animate it, or drag it onto a storyboard page.':'Drag assets onto the storyboard where you choose. Complete the storyboard details or proceed with a draft before animation.'),
    assetFiles.length?el('details',{},el('summary',{},`Project files (${assetFiles.length})`),...assetFiles.map(fileButton)):null];
  box.append(block('assets',ICONS.animate,'Assets',`${assets.length} assets`,ready("assets"),assetsBody, assetsState));
  box.append(block('animate',ICONS.animate,'Animate',`${assets.length} objects`,ready("animate"),
    el('p',{class:'why'},'Choose an asset above to animate it.'),observed('animate')?.state||'unknown'));
  if (typeof imagePromptBlock === 'function') $('magic-image-tools')?.replaceChildren(imagePromptBlock());
  if (state.view === 'design' && typeof wordsStatusRefresh === 'function') wordsStatusRefresh();
}


async function importProjectAsset(event){
 const supplied=event?.currentTarget?.uiFiles;
 const selected=supplied || await dialog.open({title:'Add artwork',multiple:true,filters:[{name:'Images',extensions:['png','jpg','jpeg','webp']}]});if(!selected)return;
 const paths=Array.isArray(selected)?selected:[selected];
 try{
  for(const path of paths){const name=path.split('/').pop().replace(/\.[^.]+$/,'');await designCommand('project.add_asset',{path,kind:'image',name,image:name,load:false});}
  state.open='assets';renderBlocks();toast('Assets saved · choose where to place them');
 }catch(e){toast(String(e));}
}
