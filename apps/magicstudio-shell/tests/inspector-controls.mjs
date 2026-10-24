// Exercise the real section helper with generic controls, including rerender and reload.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=readFileSync(new URL('../ui/animation.js',import.meta.url),'utf8'),storage=new Map();
function fixture(){
 const el=(tag,attrs={},...children)=>({tag,...attrs,children:children.flat(),events:{},addEventListener(k,fn){this.events[k]=fn}});
 const c=vm.createContext({uiConfig:JSON.parse(readFileSync(new URL('../ui/components.json',import.meta.url),'utf8')),el,localStorage:{getItem:k=>storage.get(k)??null,setItem:(k,v)=>storage.set(k,v)},window:{addEventListener(){}},ICONS:{chev:''}});vm.runInContext(source,c);return c;
}
let c=fixture();
vm.runInContext(`const a=motionSection('Action',[],true),b=motionSection('Path & depth',[]);const parent={id:'motion-inspector',querySelectorAll:()=>[a,b]};a.parentElement=b.parentElement=parent;b.open=true;b.events.toggle();`,c);
assert.equal(vm.runInContext('a.open&&b.open',c),true,'opening Path must leave Action open');
vm.runInContext(`a.open=false;a.events.toggle();`,c);
assert.equal(vm.runInContext(`motionSection('Action',[],true).open`,c),false,'closing survives inspector rerender');
assert.equal(vm.runInContext(`motionSection('Path & depth',[]).open`,c),true,'opening survives inspector rerender');
c=fixture();
assert.equal(vm.runInContext(`motionSection('Action',[],true).open`,c),false,'closing survives app reload');
assert.equal(vm.runInContext(`motionSection('Path & depth',[]).open`,c),true,'opening survives app reload');
console.log('PASS inspector: independently expandable sections persist across rerender and reload');

assert.equal(vm.runInContext(`motionSection('A dynamically named layer',[],true,'motion-selected-layer').id`,c),'motion-selected-layer','dynamic layer labels use an explicit configured component');
assert.throws(()=>vm.runInContext(`motionSection('Unconfigured panel',[])`,c),/Unconfigured/);
console.log('PASS inspector: composite canvas and dynamic layer names retain configured shared panels');
