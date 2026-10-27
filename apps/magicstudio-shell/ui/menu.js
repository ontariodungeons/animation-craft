// Native menu actions reuse the same project operations as the visible controls.
const applicationSettings = el('dialog', {id:'application-settings', 'aria-labelledby':'application-settings-title'},
  el('div', {class:'panel-head'}, el('h2', {id:'application-settings-title'}, 'Application Settings'),
    el('button', {class:'icon', title:'Close Settings', 'aria-label':'Close Settings', onclick:()=>applicationSettings.close()}, '×')),
  el('form', {id:'application-settings-form'},
    el('label', {}, 'Production project root', el('input', {id:'application-settings-root', required:'', placeholder:'/absolute/path', 'aria-label':'Production project root'})),
    el('button', {type:'button', class:'ghost', onclick:async()=>{
      try { const path=await dialog.open({title:'Production project root',directory:true,defaultPath:$('application-settings-root').value||undefined});if(path)$('application-settings-root').value=path; }
      catch(error){$('application-settings-status').textContent=String(error);}
    }}, 'Choose folder…'),
    el('p', {class:'why'}, 'Default location for new projects. Existing projects and one-project overrides are unchanged. Agent/model settings remain in Sproket.'),
    el('p', {id:'application-settings-status',role:'status'}),
    el('button', {id:'application-settings-save',type:'submit',class:'start'}, 'Save Settings')));
document.body.append(applicationSettings);

async function showApplicationSettings() {
  // The video child webview sits above DOM content; return through the existing view guard.
  if ((state.view==='studio'||document.body.classList.contains('split')) && await setView('project')===false) return;
  applicationSettings.showModal();
  $('application-settings-save').disabled=true;
  $('application-settings-status').textContent='Loading settings…';
  try {
    const settings=await invoke('engine_command',{id:'project.root',params:{}});
    $('application-settings-root').value=$('application-settings-root').defaultValue=settings.root;
    $('application-settings-status').textContent=`${settings.source} · ${settings.config}`;
    $('application-settings-save').disabled=false;
    $('application-settings-root').focus();
  } catch(error){$('application-settings-status').textContent=String(error);}
}
$('application-settings-form').addEventListener('submit',async event=>{
  event.preventDefault();
  $('application-settings-save').disabled=true;
  try {
    const settings=await invoke('engine_command',{id:'project.root',params:{root:$('application-settings-root').value}});
    $('application-settings-root').value=$('application-settings-root').defaultValue=settings.root;
    $('application-settings-status').textContent=`Saved · ${settings.config}`;
  } catch(error){$('application-settings-status').textContent=String(error);}
  finally{$('application-settings-save').disabled=false;}
});

window.studioMenu=async id=>{
  try {
    switch(id){
      case 'file.new': await beginNewProject(); break;
      case 'file.open': await openDialog(); break;
      case 'file.save':
        if(!state.project){toast('Open or create a project before saving');break;}
        if(projectView.textDrafts.size || state.data?.design?.dirty)await saveStoryboard();
        await designCommand('project.save');toast('Project saved');break;
      case 'file.close': await showHome(); break;
      case 'app.settings': await showApplicationSettings(); break;
    }
  } catch(error){toast(String(error));}
};
