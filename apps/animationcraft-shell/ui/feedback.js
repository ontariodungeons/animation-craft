// Bounded rendered feedback for this application only. Never read input values or agent chat text.
window.acFeedback={phase:'booting',errors:[],pending:0,ack:0,last:null,actions:new WeakMap()};
function feedbackSnapshot(){
 const changed=[...document.querySelectorAll('input:not([type=password]),textarea,[contenteditable=true]')].filter(n=>n.getBoundingClientRect().width&&n.getBoundingClientRect().height&&!n.closest('#agent-log')).filter(n=>n.isContentEditable?!!n.textContent.trim():n.value!==n.defaultValue).map(n=>n.id||n.tagName.toLowerCase());
 const pane=typeof state==='undefined'?null:document.querySelector(state.project?'#view-'+state.view:'#view-home');
 const rendered=!!pane&&pane.getBoundingClientRect().width>0&&pane.getBoundingClientRect().height>0&&getComputedStyle(pane).display!=='none'&&pane.textContent.trim().length>0;
 const chat=typeof agent!=="undefined"&&agent.loaded?{agent:document.getElementById('agent-command')?.value||null,provider:document.getElementById('agent-provider')?.value||null,model:document.getElementById('agent-model')?.value||null,source:'rendered selector; may include unsaved edits'}:null;
 return {chat_selector:chat,rendered,phase:acFeedback.phase,errors:acFeedback.errors.slice(-8),pending_invokes:acFeedback.pending,ack_engine_revision:acFeedback.ack,project:typeof state==='undefined'?null:state.project,view:typeof state==='undefined'?null:document.body.classList.contains('at-home')?'home':state.view,draft_fields:changed,last_frontend_command:acFeedback.last};
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

document.addEventListener('click',e=>{const n=e.target.closest('button,[role=button],[role=tab]');if(!n)return;const control=n.id||n.getAttribute('aria-label')||'unknown-control';const request_id=`ui-${Date.now()}-${Math.random().toString(16).slice(2,8)}`;e.acAction={control,request_id};acFeedback.actions.set(n,e.acAction);acFeedback.report(null,null,{stage:'ui_click',control,request_id,verified_commands:(n.dataset.commands||'').split(' ').filter(Boolean),mapping:n.dataset.commands?'verified handler annotation':'unknown'});},true);

acFeedback.acknowledge=async function(revision,project){
 await new Promise(requestAnimationFrame);
 if(state.project!==project)throw new Error('Render project mismatch; acknowledgement withheld');
 const current=feedbackSnapshot();
 if(!current.rendered||current.errors.length)throw new Error('Frontend render incomplete; acknowledgement withheld');
 await acFeedback.report('ready',revision);
};
