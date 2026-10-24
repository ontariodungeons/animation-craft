import json,subprocess,sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
css=(root/'apps/magicstudio-shell/ui/style.css').read_text()
js='''const link=document.querySelector('link[rel=stylesheet]');
const nodes=[...document.body.querySelectorAll('*')];
const keys=['display','position','width','height','minWidth','maxWidth','minHeight','maxHeight','padding','margin','borderWidth','borderRadius','fontSize','lineHeight','gap','color','backgroundColor','backgroundSize','boxShadow','transform','gridTemplateColumns','overflow'];
const sample=()=>nodes.map(n=>{const s=getComputedStyle(n);return Object.fromEntries(keys.map(k=>[k,s[k]]));});
const before=sample(), style=document.createElement('style');style.textContent=CSS;
let differences=[];
try {document.head.append(style);link.disabled=true;
const after=sample();differences=after.flatMap((s,i)=>keys.filter(k=>s[k]!==before[i][k]).map(k=>({id:nodes[i].id,rendered:before[i].display!=='none'||s.display!=='none',tag:nodes[i].tagName,class:nodes[i].className,property:k,before:before[i][k],after:s[k]})));
return {nodes:nodes.length,properties:keys.length,differences};
} finally {link.disabled=false;style.remove();}'''.replace('CSS',json.dumps(css),1)
result=json.loads(subprocess.check_output([str(root/'target/debug/mc'),'--bridge','19963','exec','ui.eval','js='+json.dumps(js)],text=True))
(root/('docs/architecture/evidence/style-equivalence-'+(sys.argv[1] if len(sys.argv)>1 else 'lyrics')+'.json')).write_text(json.dumps(result,indent=2))
print(json.dumps(result))
# Only the deliberately retired, hidden part bar lost obsolete declarations.
assert all(d['id']=='motion-part-bar' and not d['rendered'] for d in result['differences'])
