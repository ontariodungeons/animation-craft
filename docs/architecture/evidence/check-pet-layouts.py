import json,subprocess,time
from pathlib import Path
root=Path(__file__).resolve().parents[3]; out=root/'docs/architecture/evidence'
def call(c,**p):return json.loads(subprocess.check_output([str(root/'target/debug/mc'),'--bridge','19963','exec',c,*[f'{k}={json.dumps(v)}' for k,v in p.items()]],text=True))
pid=call('ui.feedback')['identity']['pid'];project=call('ui.feedback')['project'];rows=[]
call('ui.attention',project=project,id='magic-image-pane',reason='Layout acceptance')
for width in [700,941,1261]:
 subprocess.run(['hyprctl','eval',f'for _,w in ipairs(hl.get_windows()) do if w.pid == {pid} then hl.dispatch(hl.dsp.window.resize({{x={width},y=900,relative=false,window=w}})); end end'],capture_output=True,check=True)
 time.sleep(1)
 row=call('ui.eval',js='''return {width:innerWidth,errors:acFeedback.errors,pets:['sproket','gaffer'].map(name=>{const n=magicCompanions.nodes[name],p=magicCompanions.snapshot()[name],s=document.querySelector('[data-companion-slot="'+p.slot+'"]')?.getBoundingClientRect(),r=n.getBoundingClientRect();return {name,box:p.box,inside:!!s&&r.left>=s.left-1&&r.right<=s.right+1&&r.top>=s.top-1&&r.bottom<=s.bottom+1,visible:!n.hidden,error:p.placement_error};})};''')
 assert row['width']==width and not row['errors'] and all(p['inside'] and p['visible'] and not p['error'] for p in row['pets']),row
 rows.append(row);call('ui.look',output=str(out/f'pets-{width}.png'))
call('ui.attention',project=project,id='magic-image-pane',clear=True)
(out/'pet-layout-results.json').write_text(json.dumps(rows,indent=2));print(json.dumps(rows))
