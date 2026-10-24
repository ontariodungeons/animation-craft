import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import fs from 'node:fs';
function fixture(invoke) {
 const nodes=[],storage=new Map();
 const make=()=>{const n={append(){},replaceChildren(){},querySelector:()=>make()};nodes.push(n);return n;};
 const surface=make(),root=make();root.id='view-skills';root.querySelector=()=>surface;
 const $=id=>nodes.find(n=>n.id===id)||make();
 const c=vm.createContext({console,el:make,$,invoke,window:{LearningEditor:{create:async()=>({destroy:async()=>{}})}},localStorage:{getItem:k=>storage.get(k)||null,setItem:(k,v)=>storage.set(k,v),removeItem:k=>storage.delete(k)},setInterval:()=>{},navigator:{},crypto:{randomUUID:()=> 'test'}});
 vm.runInContext(fs.readFileSync(new URL('../ui/skills.js',import.meta.url),'utf8'),c);
 vm.runInContext('learningEditorState=()=>{};learningRail=()=>{};learningDemo=()=>{};learningError=()=>{};',c);
 return {c,storage};
}
test('reopening a retained draft keeps its original revision and conflict',async()=>{
 const latest={id:'a',markdown:'# Server change',revision:'new',kind:'task'};
 const {c,storage}=fixture(async()=>({skill:latest}));
 storage.set('learning:draft:/one:a',JSON.stringify({markdown:'# Local draft',revision:'old'}));
 vm.runInContext('learning.directory="/one";',c);await vm.runInContext('learningOpen("a")',c);
 assert.equal(vm.runInContext('learning.conflict?.revision',c),'new');
 assert.equal(vm.runInContext('learning.draft',c),'# Local draft');
});
test('a save response cannot clear or replace a different task/project draft',async()=>{
 let resolve;const pending=new Promise(r=>resolve=r);const {c,storage}=fixture(()=>pending);
 vm.runInContext('learning.directory="/one";learning.selected={id:"a",revision:"old"};learning.draft="# A";learning.editor={markdown:()=>"# A"};learning.epoch=1;',c);
 const saving=vm.runInContext('learningSave()',c);
 storage.set('learning:draft:/two:b',JSON.stringify({markdown:'# B unsaved',revision:'b0'}));
 vm.runInContext('learning.directory="/two";learning.selected={id:"b",revision:"b0"};learning.draft="# B unsaved";learning.epoch=2;',c);
 resolve({skill:{id:'a',markdown:'# A',revision:'a1'}});await saving;
 assert.equal(vm.runInContext('learning.selected.id',c),'b');
 assert.equal(vm.runInContext('learning.draft',c),'# B unsaved');
 assert.equal(JSON.parse(storage.get('learning:draft:/two:b')).markdown,'# B unsaved');
});
