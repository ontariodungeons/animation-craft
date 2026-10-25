// The lyric column shows each word in its line design's lettering; Shift-click picks several words and
// the Look edits reach every picked word's item.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=readFileSync(new URL('../ui/words.js',import.meta.url),'utf8');
const el=(tag,attrs={},...kids)=>{const props={};return {tag,...attrs,kids,dataset:{},style:{setProperty:(k,v)=>{props[k]=v},props},
  appendChild(n){this.kids.push(n)},prepend(n){this.kids.unshift(n)},replaceChildren(...nodes){this.kids=nodes},setAttribute(k,v){this[k]=v}}};
const c=vm.createContext({console,JSON,Map,Set,Math,Number,String,structuredClone,requestAnimationFrame(){},addEventListener(){},ResizeObserver:class{observe(){}},MutationObserver:class{observe(){}},
  window:{addEventListener(){}},document:{createElementNS:(_,tag)=>el(tag),body:{},getElementById:()=>null,addEventListener(){},createElement:()=>({getContext:()=>({measureText:()=>({width:0})}),style:{},addEventListener(){}})},state:{},$:()=>null,invoke:async()=>({}),el,ICONS:{}});
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
// Kinetic layers: an act card gives the picked words that act and saves the section's kinetic design.
const saves=[];c.invoke=async(_,r)=>{saves.push(r);return r.id==='words.save'?{animation:{source_revision:2}}:{}};c.toast=()=>{};c.wordsOpen=async()=>{};
vm.runInContext(`anim.scope={section:'verse_1',occurrence:1};anim.kinetic={layout:'scatter',entrance:'pop',overlay:'none',acts:{'l1/w2':'spin'},seed:1,style:{}};anim.source_revision=1;
  wordsPanel.read.kinetic_library={layouts:{scatter:{name:'Scatter'}},entrances:{pop:{name:'Pop'}},overlays:{none:{name:'None'}},acts:{spin:{name:'Spin'},drop:{name:'Drop'}}};
  wordsPanel.read.in_video=[];wordsPanel.picked=['l1/w0'];`,c);
const flat=(n)=>[n,...(n.kids||[]).flat(Infinity).filter((k)=>k&&typeof k==='object').flatMap(flat)];
const panel=vm.runInContext('wordsLayers(anim)',c);
const words=(n)=>typeof n==='string'?[n]:(n?.kids||[]).flat(Infinity).flatMap(words);
const drop=flat(panel).find((n)=>n.tag==='button'&&words(n).includes('Drop'));
await drop.onclick();
const save=saves.find((r)=>r.id==='words.save').params;
assert.equal(JSON.stringify(save.kinetic.acts),'{"l1/w2":"spin","l1/w0":"drop"}','the picked word gets the act; others keep theirs');
assert.equal(save.kinetic.layout,'scatter');
// Lettering and drawings: a lettering card saves its style; a stroke card anchors drawings to the picked words.
vm.runInContext(`wordsPanel.read.kinetic_library.letterings={poster:{name:'Poster',does:'',style:{font:'Nimbus Sans Narrow',hero_color:'#E4572E'}}};
  wordsPanel.read.kinetic_library.strokes={hill:{name:'Wave',does:'',view_box:'0 0 410 95',d:'M 7 77 Q 73 72 118 32'}};wordsPanel.picked=['l1/w1'];`,c);
saves.length=0;
const panel2=vm.runInContext('wordsLayers(anim)',c);
await flat(panel2).find((n)=>n.tag==='button'&&words(n).includes('Poster')).onclick();
assert.equal(saves.find((r)=>r.id==='words.save').params.kinetic.style.font,'Nimbus Sans Narrow','a lettering card sets the section style');
saves.length=0;
await flat(panel2).find((n)=>n.tag==='button'&&words(n).includes('Wave')).onclick();
assert.equal(JSON.stringify(saves.find((r)=>r.id==='words.save').params.kinetic.draws),'[{"word_id":"l1/w1","stroke":"hill","place":"under"}]','a stroke is anchored to the picked word');
// Independent graphic intervals connect only to intersecting, anchored lyric words.
vm.runInContext(`anim.kinetic.style={font:'serif',weight:400,hero_weight:800};`,c);
const intersects=vm.runInContext('wordsCueIntersects',c);
const cue={animation_id:'verse-1',start:1.3,end:3,word_ids:['l1/w1']};
assert.equal(intersects(cue,'verse-1',0,4,['l1/w1']),true);
assert.equal(intersects(cue,'verse-1',0,1.3,['l1/w1']),true,'the meaning link survives moving a drawing after its word');
assert.equal(intersects(cue,'verse-1',3,4,['l1/w1']),true,'the meaning link survives an early drawing');
assert.equal(intersects(cue,'verse-2',0,4,['l1/w1']),false,'another section does not inherit the connection');
assert.equal(intersects(cue,'verse-1',0,4,['l1/w0']),false,'another word does not inherit the drawing');
assert.equal(intersects({...cue,word_ids:[]},'verse-1',0,4,['l1/w0']),true,'section overlays intersect every overlapping line');
const wave=vm.runInContext("wordsStroke(wordsPanel.read.kinetic_library.strokes.hill)",c);
assert.equal(wave.kids[0].d,'M 7 77 Q 73 72 118 32','visual cue uses the actual library path');
// The Design view's own width picks its layout: narrow retains both tracks; tight puts the stage above the two tracks.
for (const [width, narrow, tight] of [[1400, false, false], [900, true, false], [401, true, true]]) {
  const classes = new Set(); const view = { clientWidth: width, classList: { toggle: (k, on) => on ? classes.add(k) : classes.delete(k) } };
  vm.runInContext('wordsWidth', c)(view);
  assert.equal(classes.has('narrow'), narrow, `narrow at ${width}px`); assert.equal(classes.has('tight'), tight, `tight at ${width}px`);
}
console.log('PASS words lettering: column words use the line design; Shift-click picks several; text locked; kinetic act cards save the picked words');

// Option diagrams show distinct arrangements and label every choice for keyboard/bridge access.
const cascade=vm.runInContext("wordsOptionPreview('layouts','cascade')",c);
assert.equal(cascade.kids[0].y,'19'); assert.equal(cascade.kids[2].y,'49');
assert.equal(flat(panel2).find(n=>n.tag==='button'&&words(n).includes('Poster'))['aria-label'],'Poster');
assert.equal(chip(0).style.props['--word-weight'],'400','kinetic lettering also reaches the lyric column');
// Execute the production composition script: drawing tween must use its own word time.
const library=JSON.parse(readFileSync(new URL('../../../pipeline/templates/kinetic-library.json',import.meta.url),'utf8'));
const make=(tag)=>{const n=el(tag);Object.assign(n,{offsetWidth:100,offsetHeight:60,offsetLeft:20,offsetTop:40,getTotalLength:()=>240});
 n.appendChild=function(child){this.kids.push(child);child.parentElement=this};return n};
const root=make('div');root.querySelector=(selector)=>flat(root).find(n=>selector.includes(n.dataset?.wordId)&&n.dataset?.wordId);
const tweens=[];const data={width:1920,height:1080,duration:8,words:[{word_id:'a',line_id:'line',text:'Sing',start:2,end:3}],
 kinetic:{layout:'karaoke',entrance:'pop',draws:[{word_id:'a',stroke:'hill',place:'under'}]},style:{size:64,hero_size:110,hero_color:'#B8442A'},library};
const template=readFileSync(new URL('../../../pipeline/templates/words-kinetic.html',import.meta.url),'utf8');
vm.runInNewContext(template.match(/<script>([\s\S]*?)<\/script>/)[1].replace('__DATA__',JSON.stringify(data)),{
 document:{querySelector:()=>root,createElement:make,createElementNS:(_,tag)=>make(tag)},CSS:{escape:s=>s},window:{},
 gsap:{timeline:()=>({fromTo:(target,from,to,time)=>tweens.push({target,from,to,time})})}});
const drawTween=tweens.find(t=>t.target.tag==='path');
assert.equal(drawTween.time,2,'stroke begins at the anchored sung word');
assert.equal(drawTween.from.strokeDashoffset,240);assert.equal(drawTween.to.strokeDashoffset,0);
assert.equal(drawTween.target.d,library.strokes.hill.d,'export uses the same stroke as the film frame');
assert.equal(drawTween.target.parentElement.parentElement,root.kids[0],'drawing leaves with its lyric line');
console.log('PASS kinetic previews and anchored drawing export timing');

const previewRange=vm.runInContext('wordsCueWindow',c)({...cue,start:4,end:6},{start:1,end:3});
assert.deepEqual(JSON.parse(JSON.stringify(previewRange)),{start:.75,end:6.25},'replay includes both the lyric and its delayed drawing');
assert.equal(intersects({...cue,word_ids:[]},'verse-1',3,4,['l1/w1']),false,'unanchored effects still use interval intersections');

const editLine=vm.runInContext('wordsPresentationEdits',c);
const anchors=[{word_id:'a',text:'Three'},{word_id:'b',text:'times'},{word_id:'c',text:'around'}];
assert.equal(JSON.stringify(editLine(anchors,'Three happy times around')),'[{"word_id":"a","display_text":"Three happy"},{"word_id":"b","display_text":"times"},{"word_id":"c","display_text":"around"}]','inserted text preserves the original timed IDs');
assert.equal(JSON.stringify(editLine(anchors,'Three around')),'[{"word_id":"a","display_text":"Three"},{"word_id":"b","display_text":""},{"word_id":"c","display_text":"around"}]','deletion keeps timing anchors available');
assert.equal(editLine(anchors,'Two times around')[0].display_text,'Two');
assert.equal(anchors[0].text,'Three','presentation editing never mutates the source lyric');
assert.throws(()=>editLine(anchors,'x '.repeat(300)),/256 words/);
console.log('PASS inline presentation edits preserve audio and word anchors');
