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
