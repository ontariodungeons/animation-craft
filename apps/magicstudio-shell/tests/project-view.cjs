const {test} = require('node:test');
const assert = require('node:assert/strict');
const {readFileSync} = require('node:fs');
const {runInNewContext} = require('node:vm');

function fixture(invoke) {
  const state = {project:'/song/Song.magic', data:{}, page:0};
  const context = {state, invoke, updateAppFooter(){}, renderTarget(){}, renderBlocks(){}, renderStoryboard(){}, acFeedback:{acknowledge:async()=>{}}};
  runInNewContext(readFileSync(__dirname+'/../ui/project.js','utf8')+'\nglobalThis.controller = projectView;', context);
  context.renderBlocks=()=>{};
  return {state, controller:context.controller};
}

test('project commands serialize writes and keep the latest returned state', async()=>{
  const calls=[];
  let finish;
  const {state,controller}=fixture(async(_,args)=>{
    calls.push(args.id);
    if(args.id==='first')await new Promise(resolve=>finish=resolve);
    return {project:args.project, marker:args.id};
  });
  const first=controller.command('first');
  const second=controller.command('second');
  await new Promise(setImmediate);
  assert.deepEqual(calls,['first']);
  finish();await Promise.all([first,second]);
  assert.deepEqual(calls,['first','second']);
  assert.equal(state.data.marker,'second');
  assert.equal(controller.pending,0);
});

test('a late reply never replaces a different open project', async()=>{
  let finish;
  const {state,controller}=fixture(async()=>new Promise(resolve=>finish=resolve));
  const operation=controller.command('storyboard.edit');
  await new Promise(setImmediate);
  state.project='/other/Other.magic';state.data={marker:'other'};
  finish({marker:'old'});
  await assert.rejects(operation,/project changed/i);
  assert.equal(state.data.marker,'other');
});

test('a failed command retains edits and does not poison subsequent writes', async()=>{
  const {state,controller}=fixture(async(_,args)=>{if(args.id==='fail')throw Error('disk full');return {marker:'saved'};});
  state.data={design:{dirty:true}};
  await assert.rejects(controller.command('fail'),/disk full/);
  assert.equal(state.data.design.dirty,true);
  assert.equal(controller.error,'disk full');
  await controller.command('retry');
  assert.equal(controller.error,null);
  assert.equal(state.data.marker,'saved');
});

test('the selected scene follows the visible page and engine readiness is preserved',()=>{
  const {state,controller}=fixture();
  state.storyboardScene='first';state.page=4;
  state.data={storyboard_contents:{chapters:[{target:'first',page:2},{target:'second',page:5}]},monitoring:{prerequisites:[{id:'song',state:'failed',reason:'missing recording'}]}};
  assert.equal(controller.scene().target,'second');
  assert.equal(controller.prerequisite('song').state,'failed');
  assert.equal(controller.prerequisite('song').reason,'missing recording');
});
