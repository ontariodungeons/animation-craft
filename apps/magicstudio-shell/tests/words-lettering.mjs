// The lyric column shows each word in its line design's lettering; Shift-click picks several words and
// the Look edits reach every picked word's item.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=readFileSync(new URL('../ui/words.js',import.meta.url),'utf8');
const el=(tag,attrs={},...kids)=>{const props={};return {tag,...attrs,kids,dataset:{},style:{setProperty:(k,v)=>{props[k]=v},props}}};
const c=vm.createContext({console,JSON,Map,Set,Math,Number,String,structuredClone,requestAnimationFrame(){},addEventListener(){},ResizeObserver:class{observe(){}},MutationObserver:class{observe(){}},
  window:{addEventListener(){}},document:{body:{},getElementById:()=>null,addEventListener(){},createElement:()=>({getContext:()=>({measureText:()=>({width:0})}),style:{},addEventListener(){}})},state:{},$:()=>null,invoke:async()=>({}),el,ICONS:{}});
vm.runInContext(source,c);
let renders=0;c.wordsRender=()=>{renders++};
vm.runInContext(`

  var anim={animation_id:'verse-1',style:{font:'Fredoka'},words:[
    {word_id:'l1/w0',line_id:'l1',text:'Three',start:0.4},{word_id:'l1/w1',line_id:'l1',text:'times',start:1.3},{word_id:'l1/w2',line_id:'l1',text:'around',start:2.1}],
    design:{l1:{items:[{word_ids:['l1/w0'],font:'song',weight:700,italic:false,color:'#c0392b',size:220},
                       {word_ids:['l1/w1'],font:'serif',weight:400,italic:true,color:'#123456',size:55},
                       {word_ids:['l1/w2'],font:'sans',weight:400,italic:false,color:'#000000',size:110}],draws:[]}}};
  wordsPanel.read={document:{animations:[anim]},fonts:[{family:'Fredoka'}]};wordsPanel.selected='verse-1';
`,c);
const chip=(i)=>vm.runInContext(`wordsChip(anim, anim.words[${i}])`,c);
const first=chip(0).style.props;
assert.equal(first['--word-font'],'Fredoka','the song font is the section font');
assert.equal(first['--word-weight'],'700');assert.equal(first['--word-color'],'#c0392b');assert.equal(first['--word-scale'],'1.70','size scales, capped');
assert.equal(chip(1).style.props['--word-slant'],'italic');assert.equal(chip(1).style.props['--word-scale'],'0.80');
const click=(i,shiftKey)=>chip(i).onclick({stopPropagation(){},shiftKey});
click(0,false);click(2,true);
assert.equal(vm.runInContext('JSON.stringify(wordsPanel.picked)',c),'["l1/w0","l1/w2"]','Shift-click adds a word in the same line');
assert.equal(vm.runInContext('JSON.stringify(wordsPickedItems(anim.design.l1))',c),'[0,2]','Look edits reach every picked item');
click(2,true);
assert.equal(vm.runInContext('JSON.stringify(wordsPanel.picked)',c),'["l1/w0"]','Shift-click on a picked word drops it');
click(1,false);
assert.equal(vm.runInContext('JSON.stringify(wordsPickedItems(anim.design.l1))',c),'[1]','a plain click picks one word');
assert.equal(vm.runInContext('anim.words.map((w)=>w.text).join(" ")',c),'Three times around','the printed text is never edited');
console.log('PASS words lettering: column words use the line design; Shift-click picks several; text locked');
