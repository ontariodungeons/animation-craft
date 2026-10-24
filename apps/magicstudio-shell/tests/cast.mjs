// Exercise the actual save handler: delayed replies must preserve later edits and navigation.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const source=readFileSync(new URL('../ui/cast.js',import.meta.url),'utf8');
const nodes=new Map(['learning-save-state','learning-save','learning-message','cast-saved-file'].map(id=>[id,{textContent:'',querySelector(){return this;}}]));
const stored=new Map();let reply;
const ctx=vm.createContext({structuredClone,console,Map,JSON,
  $:id=>nodes.get(id),learningStored:key=>stored.get(key),learningStore:(key,v)=>v===null?stored.delete(key):stored.set(key,structuredClone(v)),
  invoke:()=>new Promise(resolve=>{reply=resolve;})});
vm.runInContext(source,ctx);
vm.runInContext(`cast.workspace='/work';cast.active=true;cast.items=[{revision:'r1',path:'/work/creative/AGENTS.md',markdown:'original',document:{department:{id:'creative'},instructions:'original'}}];`,ctx);
vm.runInContext(`castDraft().document.instructions='first edit';castChanged();`,ctx);
const first=vm.runInContext('castSave()',ctx);
assert.equal(nodes.get('learning-save').disabled,true);
vm.runInContext(`castDraft().document.instructions='typed during save';castChanged();`,ctx);
reply({department:{revision:'r2',path:'/work/creative/AGENTS.md',markdown:'first edit',document:{department:{id:'creative'},instructions:'first edit'}}});
await first;
assert.equal(vm.runInContext('castDraft().document.instructions',ctx),'typed during save');
assert.equal(vm.runInContext('castDraft().revision',ctx),'r2');
assert.equal(stored.get('cast:/work:creative').document.instructions,'typed during save');
assert.equal(nodes.get('learning-save').disabled,false);

// A successful save clears exactly that draft; the native revision is retained.
const second=vm.runInContext('castSave()',ctx);
reply({department:{revision:'r3',path:'/work/creative/AGENTS.md',markdown:'typed during save',document:{department:{id:'creative'},instructions:'typed during save'}}});
await second;
assert.equal(stored.has('cast:/work:creative'),false);
assert.equal(nodes.get('learning-save-state').textContent,'Saved');

// Moving to another workspace while a response is in flight cannot replace its data.
vm.runInContext(`castDraft().document.instructions='old workspace';`,ctx);
const third=vm.runInContext('castSave()',ctx);
vm.runInContext(`cast.active=false;cast.workspace='/other';cast.items=[];`,ctx);
reply({department:{revision:'wrong',document:{department:{id:'creative'},instructions:'wrong'}}});await third;
assert.equal(vm.runInContext('cast.items.length',ctx),0);

// An external edit is shown as the saved version while our local draft survives.
vm.runInContext(`cast.active=true;cast.workspace='/work';cast.items=[{revision:'r3',path:'/work/creative/AGENTS.md',markdown:'saved',document:{department:{id:'creative'},instructions:'saved'}}];`,ctx);
ctx.invoke=async(_,request)=>{
  if(request.id==='crew.save')throw Error('Cast conflict');
  return {departments:[{revision:'external',path:'/work/creative/AGENTS.md',markdown:'external edit',document:{department:{id:'creative'},instructions:'external edit'}}]};
};
await vm.runInContext('castSave()',ctx);
assert.equal(vm.runInContext('castDraft().document.instructions',ctx),'old workspace');
assert.equal(vm.runInContext('castItem().revision',ctx),'external');
assert.match(nodes.get('learning-message').textContent,/conflict.*draft is retained/);
console.log('PASS cast: delayed saves retain edits, conflicts and workspace ownership');
