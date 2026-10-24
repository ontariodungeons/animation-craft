// Execute the actual placement renderer against saved native decks with no image overlays.
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";
const source=readFileSync(new URL("../ui/app.js",import.meta.url),"utf8");
const renderer=source.slice(source.indexOf("function renderDesignPlacements(){"),source.indexOf("\n$('sb-save-version').addEventListener"));
const button={hidden:true};
const state={data:{storyboard:{deck:"/fixture/movie-board.deckcraft"},design:{placements:[]}}};
const context=vm.createContext({state,$:()=>button,document:{querySelectorAll:()=>[]},selectedPlacement:null});
vm.runInContext(renderer,context);
vm.runInContext("renderDesignPlacements()",context);
assert.equal(button.hidden,false,"Saved native deck must expose Save even before any image is placed");
state.data.design={placements:[],dirty:true};
vm.runInContext("renderDesignPlacements()",context);
assert.equal(button.hidden,false,"Removing the final image must leave Save reachable");
state.data.storyboard={previews:["/fixture/legacy.png"]};
vm.runInContext("renderDesignPlacements()",context);
assert.equal(button.hidden,true,"Legacy preview without a native deck must not advertise native Save");
console.log("PASS: native Save remains reachable with zero placements and after final-image removal; legacy previews stay distinct");

const rendered=[];
const designContext=vm.createContext({state:{project:"/fixture/test.magic",data:{}},
 motionCommand:async()=>({_ui_engine_revision:2}),invoke:async()=>({}),
 renderBlocks:()=>rendered.push("blocks"),renderStoryboard:()=>rendered.push("storyboard"),
 acFeedback:{acknowledge:async(revision,project)=>{assert.deepEqual(rendered,["blocks","storyboard"]);assert.equal(revision,2);assert.equal(project,"/fixture/test.magic");rendered.push("ack");}}});
vm.runInContext(source.slice(source.indexOf("async function designCommand("),source.indexOf("async function importProjectAsset(")),designContext);
await vm.runInContext("designCommand('storyboard.save_version')",designContext);
assert.deepEqual(rendered,["blocks","storyboard","ack"],"Save must acknowledge its engine revision after rendering the returned state");
console.log("PASS: native Save acknowledges the rendered project revision");

// Rendering after Save must retain the actual selected page, not just its toolbar number.
const scrolled=[],nodes=new Map();
function dom(id){return {id,children:[],dataset:{},style:{setProperty(){}},classList:{toggle(){},add(){}},replaceChildren(){this.children=[];},append(...n){this.children.push(...n);},querySelector(){return null;},addEventListener(){},remove(){},scrollIntoView(){scrolled.push(this.id);},};}
const get=id=>{if(!nodes.has(id))nodes.set(id,dom(id));return nodes.get(id);};
const renderContext=vm.createContext({updateAppFooter(){},state:{project:'/fixture/test.magic',page:1,data:{storyboard:{template:'backlot',previews:['one','two','three'],scene_titles:['scene-1','scene-2'],first_scene_page:2}}},
 $:get,document:{body:{classList:{toggle(){}}}},el:(tag,attrs={})=>dom(attrs.id),convertFileSrc:p=>p,renderDesignPlacements(){},renderStoryboardText(){}});
vm.runInContext(source.slice(source.indexOf('function renderStoryboard() {'),source.indexOf('function showPage(')),renderContext);
vm.runInContext('renderStoryboard()',renderContext);
assert.equal(scrolled.at(-1),'sb-sheet-1','Refresh must show the selected slide after rebuilding its page container');
renderContext.state.page=2;vm.runInContext('renderStoryboard()',renderContext);
assert.equal(scrolled.at(-1),'sb-sheet-2');
console.log('PASS: storyboard refresh retains the visibly selected page');

const labels=[];renderContext.el=(tag,attrs={},...children)=>{if(tag==='button'&&attrs.role==='tab')labels.push(children[0]);return dom(attrs.id);};
renderContext.state.data.storyboard.page_titles=['Song story & backgrounds','Contents','Scene 01'];
vm.runInContext('renderStoryboard()',renderContext);
assert.deepEqual(labels,['Song story & backgrounds','Contents','Scene 01'],'Navigation must use document headings when supplied, without per-song button maps');
console.log('PASS: document-derived page headings populate the existing runtime navigation loop');
