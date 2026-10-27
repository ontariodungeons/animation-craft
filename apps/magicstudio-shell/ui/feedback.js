// Bounded rendered feedback for this application only. Never read input values or agent chat text.
window.acFeedback={phase:'booting',errors:[],pending:0,ack:0,last:null,actions:new WeakMap()};
function feedbackSnapshot(){
 const changed=[...document.querySelectorAll('input:not([type=password]),textarea,[contenteditable=true]')].filter(n=>n.getBoundingClientRect().width&&n.getBoundingClientRect().height&&!n.closest('#agent-log')&&n.id!=='motion-scrub').filter(n=>n.isContentEditable?!!n.textContent.trim():n.type==='checkbox'||n.type==='radio'?n.checked!==n.defaultChecked:n.value!==n.defaultValue).map(n=>n.id||n.tagName.toLowerCase());
 if(typeof projectView!=='undefined'){changed.push(...projectView.textDrafts.keys());if(state.data?.design?.dirty===true)changed.push('storyboard-design');}
 if(typeof theatreFeedback==='function'&&theatreFeedback().live_edits==='needs_save')changed.push('theatre-motion');
 const pane=typeof state==='undefined'?null:document.querySelector(state.project||state.creating?'#view-'+(state.view==='studio'&&typeof serverUrl==='function'&&serverUrl()?'project':state.view):'#view-home');
 const rendered=!!pane&&pane.getBoundingClientRect().width>0&&pane.getBoundingClientRect().height>0&&getComputedStyle(pane).display!=='none'&&pane.textContent.trim().length>0;
 const chat=typeof agent!=="undefined"&&agent.loaded?{agent:document.getElementById('agent-command')?.value||null,provider:document.getElementById('agent-provider')?.value||null,model:document.getElementById('agent-model')?.value||null,source:'rendered selector; may include unsaved edits'}:null;
 // The shared box snapshot rides in the EXISTING feedback payload, so the app
 // light, the companion and the TUI all read ONE observation. Each row says
 // where it came from and when it was seen; a stale or closed-app observation
 // is marked, never invented as a failure. No new poller and no new command.
 // The one standard record per panel: flags, a revision that changes only when
 // a flag changes, and the clickable controls that belong to no known panel.
 const panels=window.companionBoxes?window.companionBoxes.panels():null;
 return {companions:window.magicCompanions?.snapshot()||null,panels,theatre:typeof theatreFeedback==='function'?theatreFeedback():null,chat_selector:chat,rendered,phase:acFeedback.phase,errors:acFeedback.errors.slice(-8),pending_invokes:acFeedback.pending,ack_engine_revision:acFeedback.ack,project:typeof state==='undefined'?null:state.project,view:typeof state==='undefined'?null:document.body.classList.contains('at-home')?'home':state.view,draft_fields:changed,last_frontend_command:acFeedback.last};
}
acFeedback.report=async function(phase,ack,event){
 if(phase)acFeedback.phase=phase;if(ack)acFeedback.ack=Math.max(acFeedback.ack,ack);
 try{await window.__TAURI__.core.invoke('ui_report',{report:{...feedbackSnapshot(),...(event?{event}: {})}});}catch(e){console.error('UI feedback failed',String(e));}
};
window.addEventListener('error',e=>{acFeedback.errors.push({message:String(e.message).slice(0,300),source:String(e.filename).split('/').pop(),line:e.lineno});acFeedback.report('error');});
window.addEventListener('unhandledrejection',e=>{acFeedback.errors.push({message:String(e.reason).slice(0,300)});acFeedback.report('error');});
let feedbackTimer;
document.addEventListener('input',()=>{clearTimeout(feedbackTimer);feedbackTimer=setTimeout(()=>acFeedback.report(),150);});
acFeedback.report('booting');
// Refresh observation health without creating activity or acknowledging new engine work.
setInterval(()=>acFeedback.report(),1000);

document.addEventListener('click',e=>{
 const n=e.target.closest('button,[role=button],[role=tab]');if(!n)return;
 const control=n.id||n.getAttribute('aria-label')||n.getAttribute('title')||n.textContent.trim().slice(0,80)||'unknown-control';
 const request_id=`ui-${Date.now()}-${Math.random().toString(16).slice(2,8)}`;
 const navigation=n.matches('#project-details,#sb-prev,#sb-next,.sb-link,#sb-chips [role=tab],.tab[data-view]');
 const previous=acFeedback.action,errors=acFeedback.errors.length;
 e.acAction={control,request_id,box:n.closest("[data-companion-rail]")?.getAttribute("data-companion-rail")||n.closest("[data-box-id]")?.getAttribute("data-box-id")||null};acFeedback.actions.set(n,e.acAction);acFeedback.action=e.acAction;
 // Static handlers run during this dispatch too; invoke captures this context before awaiting.
 queueMicrotask(()=>{if(acFeedback.action===e.acAction)acFeedback.action=previous;});
 acFeedback.report(null,null,{stage:'ui_click',control,request_id,interaction:navigation?'navigation':'control',verified_commands:(n.dataset.commands||'').split(' ').filter(Boolean),mapping:navigation?'existing navigation handler':n.dataset.commands?'verified handler annotation':'unknown'});
 if(navigation&&!n.matches('.tab[data-view]'))requestAnimationFrame(()=>{
  const current=feedbackSnapshot();
  if(current.rendered&&acFeedback.errors.length===errors)acFeedback.report(null,null,{stage:'ui_navigation_applied',control,request_id,view:current.view,page:state.page+1});
 });
},true);

acFeedback.acknowledge=async function(revision,project){
 const frameObserved=await Promise.race([
  new Promise(resolve=>requestAnimationFrame(()=>resolve(true))),
  new Promise(resolve=>setTimeout(()=>resolve(false),800)),
 ]);
 if(state.project!==project)throw new Error('Render project mismatch; acknowledgement withheld');
 const current=feedbackSnapshot();
 if(!current.rendered||current.errors.length)throw new Error('Frontend render incomplete; acknowledgement withheld');
 if(!frameObserved){
  await acFeedback.report('ready',null,{stage:'render_deferred',project,reason:'No animation frame observed; engine revision acknowledgement withheld'});
  return;
 }
 await acFeedback.report('ready',revision);
};
