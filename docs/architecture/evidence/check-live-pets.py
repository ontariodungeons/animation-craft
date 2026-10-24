import json, subprocess, time
from pathlib import Path
root=Path(__file__).resolve().parents[3]; out=root/'docs/architecture/evidence'
def call(command, **params):
    r=subprocess.run([str(root/'target/debug/mc'),'--bridge','19963','exec',command,*[f'{k}={json.dumps(v)}' for k,v in params.items()]],capture_output=True,text=True)
    if r.returncode: raise RuntimeError(r.stderr)
    return json.loads(r.stdout)
def read(js): return call('ui.eval',js=js)
def wait(js):
    value=None
    for _ in range(100):
        value=read(js)
        if value: return value
        time.sleep(.1)
    raise AssertionError(value)
def snapshot():return read('return magicCompanions.snapshot();')
def photo(name):call('ui.look',output=str(out/(name+'.png')))
for _ in range(100):
    feedback=call('ui.feedback')
    if feedback.get('ui_ready') and feedback.get('project'):break
    time.sleep(.1)
else:raise AssertionError('Studio did not finish rendering its project')
project=feedback['project']; results={'project':project,'identity':feedback['identity']}
assert feedback['identity']['executable']==str(root/'target/debug/magicstudio-shell')
pid=feedback['identity']['pid']
windows=json.loads(subprocess.check_output(['hyprctl','clients','-j']))
window=next(w for w in windows if w['pid']==pid)
if not window['floating']:
    subprocess.run(['hyprctl','eval',f'for _,w in ipairs(hl.get_windows()) do if w.pid == {pid} then hl.dispatch(hl.dsp.window.float({{action="set",window=w}})); end end'],check=True,capture_output=True)
    time.sleep(.5)
subprocess.run(['hyprctl','eval',f'for _,w in ipairs(hl.get_windows()) do if w.pid == {pid} then hl.dispatch(hl.dsp.window.resize({{x=1261,y=900,relative=false,window=w}})); end end'],check=True,capture_output=True)
time.sleep(.5)

assert snapshot()['enabled']
assets=read('''const m=await (await fetch('companions.manifest.json')).json();return await Promise.all(Object.values(m.characters).flatMap(c=>Object.values(c.poses)).map(async src=>{const img=new Image();img.src=src;await img.decode();return {src,width:img.naturalWidth,height:img.naturalHeight};}));''')
assert len(assets)==10 and all(a['src'].endswith('.svg') and a['width']>0 and a['height']>0 for a in assets);results['assets']=assets
call('ui.attention',project=project,id='magic-image-pane',reason='TUI attention proof')
wait('return magicCompanions.snapshot().sproket.box==="magic-image-pane";');time.sleep(1)
results['attention']=snapshot();photo('pets-tui-attention')
call('ui.attention',project=project,id='magic-image-pane',clear=True)
failure=read('''try{await invoke('engine_command',{id:'ui_contract_missing_command',params:{_control:'agent-send'}});return null;}catch(error){return String(error);}''')
assert failure and 'unknown' in failure.lower(),failure
wait('return magicCompanions.snapshot().sproket.box==="magic-chat-pane" && magicCompanions.snapshot().sproket.signal===3;');time.sleep(1)
results['actual_failure']={'error':failure,'pets':snapshot()};photo('pets-actual-error')
read('''await invoke('engine_command',{id:'project.identity',params:{path:state.project,_control:'agent-send'}});return true;''')
wait('return magicCompanions.snapshot().gaffer.box==="magic-chat-pane" && uiRegistry.get("magic-chat-pane").state==="ready";');results['last_chat_action']=snapshot()
read('''const n=document.createElement('button');n.id='contract-proof-missing';n.textContent='Contract test fixture';n.setAttribute('data-box-id','contract-proof-unregistered');document.getElementById('magic-chat-pane').append(n);uiRegistry.panels();return true;''')
wait('return magicCompanions.snapshot().sproket.box==="magic-chat-pane" && magicCompanions.snapshot().sproket.signal===3;')
results['missing_state']={'pets':snapshot(),'diagnostics':read('return magicBridge.unconfigured();')};assert any(r['id']=='contract-proof-missing' for r in results['missing_state']['diagnostics']);photo('pets-missing-state-red')
read('document.getElementById("contract-proof-missing").remove();uiRegistry.panels();return true;')
wait('return !magicBridge.unconfigured().length && uiRegistry.panels().items.find(r=>r.id==="magic-chat-pane").signal!==3;')
call('ui.click',id='agent-config-toggle');call('ui.set',id='ambient-companions',value=False)
assert read('return !magicCompanions.snapshot().enabled && Object.values(magicCompanions.nodes).every(n=>n.hidden);')
call('ui.set',id='ambient-companions',value=True);call('ui.click',id='agent-config-toggle')
assert snapshot()['enabled'];results['disable_and_reenable']=True
# Settings load model metadata asynchronously; idle begins after those real actions finish.
wait('return acFeedback.pending===0;');time.sleep(.2);wait('return acFeedback.pending===0;')
# Controlled fixture: exercise idle homes without claiming the empty project meets intake prerequisites.
read('''for(const row of uiRegistry.panels().items)uiRegistry.fact({id:row.id,state:'idle',source:'acceptance fixture'});return true;''')
wait('return magicCompanions.snapshot().sproket.box===null && magicCompanions.snapshot().gaffer.box===null && magicCompanions.snapshot().sproket.slot==="magic-chat-pane" && magicCompanions.snapshot().gaffer.slot==="magic-image-pane";');time.sleep(1)
results['idle_fixture']=snapshot();(out/'live-pets-results.json').write_text(json.dumps(results,indent=2));assert results['idle_fixture']['sproket']['slot']=='magic-chat-pane' and results['idle_fixture']['gaffer']['slot']=='magic-image-pane';photo('pets-idle-homes')
results['geometry']=read('''return ['sproket','gaffer'].map(name=>{const pet=magicCompanions.snapshot()[name],n=magicCompanions.nodes[name],slot=document.querySelector('[data-companion-slot="'+pet.slot+'"]'),r=n.getBoundingClientRect(),s=slot.getBoundingClientRect(),img=n.querySelector('img');return {name,slot:pet.slot,visible:!n.hidden,image_loaded:img.complete&&img.naturalWidth>0,inside:r.x>=s.x-1&&r.y>=s.y-1&&r.right<=s.right+1&&r.bottom<=s.bottom+1,index:n.dataset.uiIndex};});''')
assert all(r['inside'] and r['image_loaded'] and r['visible'] and r['index'] for r in results['geometry']),results['geometry']
assert not call('ui.feedback')['errors']
(out/'live-pets-results.json').write_text(json.dumps(results,indent=2));print(json.dumps({k:v for k,v in results.items() if k in ['geometry','disable_and_reenable']}))
