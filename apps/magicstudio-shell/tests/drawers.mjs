import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

// Real controller, with fixed geometry: opening one assistant cannot rebuild or empty another.
const events={},nodes=new Map(),layouts=[],changes=[];
function node(id,dataset={}){
 const n={id,dataset,hidden:true,attrs:{},value:'',focused:false,
  setAttribute(k,v){this.attrs[k]=v;},focus(){this.focused=true;},
  classList:{contains(){return false;}},
  closest(selector){return selector==='[data-drawer-toggle]'&&this.dataset.drawerToggle?this:null;}};
 nodes.set(id,n);return n;
}
const host=node('magic-workstation');host.getBoundingClientRect=()=>({height:host.hidden?0:320});host.contains=n=>n.parent===host;
const chat=node('chat',{drawer:'sproket'}),art=node('art',{drawer:'gaffer'});chat.parent=host;art.parent=host;
const chatToggle=node('chat-toggle',{drawerToggle:'sproket'}),artToggle=node('art-toggle',{drawerToggle:'gaffer'});
const root={getElementById:id=>nodes.get(id),addEventListener:(id,fn)=>{events[id]=fn;},querySelectorAll:selector=>selector==='[data-drawer]'?[chat,art]:[chatToggle,artToggle]};
const c=vm.createContext({document:root,window:{addEventListener(){}},ResizeObserver:class{observe(){}},console});
vm.runInContext(readFileSync(new URL('../ui/drawers.js',import.meta.url),'utf8'),c);
c.options={root,onLayout:h=>layouts.push(h),onChange:(...args)=>changes.push(args)};
const api=vm.runInContext('createTopDrawers(options)',c);
assert.equal(api.active,null);assert.deepEqual(layouts,[0]);
api.open('sproket');chat.value='Retain my message';
assert.equal(host.hidden,false);assert.equal(chat.hidden,false);assert.equal(art.hidden,true);
assert.equal(chatToggle.attrs['aria-expanded'],'true');
events.click({target:artToggle});
assert.equal(api.active,'gaffer');assert.equal(chat.hidden,true);assert.equal(art.hidden,false);
assert.equal(chat.value,'Retain my message');assert.equal(chatToggle.attrs['aria-expanded'],'false');
assert.equal(api.open('invented'),false);assert.equal(api.active,'gaffer');
let prevented=false;events.keydown({key:'Escape',preventDefault(){prevented=true;}});
assert.equal(prevented,true);assert.equal(api.active,null);assert.equal(host.hidden,true);assert.equal(artToggle.focused,true);
assert.equal(layouts.at(-1),0,'Closing restores the full native Studio viewport');
api.open('sproket');events.click({target:{closest(){return null;}}});
assert.equal(api.active,null);assert.equal(chat.value,'Retain my message');
assert.equal(changes.length,5);
console.log('PASS drawers: one open panel, retained drafts, Escape, outside close and native layout');
