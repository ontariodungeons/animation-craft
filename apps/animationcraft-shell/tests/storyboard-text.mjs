import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../ui/app.js',import.meta.url),'utf8');
assert.ok(source.includes('function renderStoryboardText(){'),'native storyboard text needs a UI binding');
const nodes=[],calls=[];
const context=vm.createContext({state:{data:{storyboard:{page_size:[1080,1600]},design:{text_fields:[{page:18,scene:14,field:'action',id:1486,rect:[78,1026,428,50.4],value:'Original action'}]}}},
 $:()=>({append:n=>nodes.push(n)}),document:{querySelectorAll:()=>[]},
 el:(tag,attrs)=>({tag,attrs,handlers:{},addEventListener(event,fn){this.handlers[event]=fn;}}),
 designCommand:async(id,params)=>calls.push({id,params}),toast(){}});
vm.runInContext(source.slice(source.indexOf('function renderStoryboardText(){'),source.indexOf('async function designCommand(')),context);
vm.runInContext('renderStoryboardText()',context);
assert.equal(nodes[0].value,'Original action');
assert.equal(nodes[0].defaultValue,'Original action');
nodes[0].value='Changed action';await nodes[0].handlers.change();
assert.equal(calls[0].id,'storyboard.edit');
assert.equal(calls[0].params.page,18);assert.equal(calls[0].params.field,'action');
assert.equal(calls[0].params.value,'Changed action');
console.log('PASS: native field maps to exact existing storyboard.edit page and field');

context.state.data.design.text_fields[0]={page:13,scene:4,field:'action',id:1171,name:'page_13_user_scene_action',font_size:12,rect:[78,1026,428,114],value:'Same PDF text',fill:'#1B1D22',text_color:'#B7B4AB'};
vm.runInContext('renderStoryboardText()',context);
const style=nodes.at(-1).attrs.style;
assert.ok(style.includes('font-family:Helvetica,Arial,sans-serif'));
assert.ok(style.includes(`font-size:${12/1080*100}cqw`));
assert.ok(style.includes(`height:${114/1600*100}%`));
console.log('PASS: PDF input keeps the displayed field font size, family and bounds');
