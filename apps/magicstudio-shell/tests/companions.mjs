// Companion pets follow signals. No command ids, no job correlation, no pin.
// Loads the real companion-boxes.js and companions.js in a vm with a stub DOM.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = name => readFileSync(new URL(`../ui/${name}`, import.meta.url), 'utf8');

function fixture() {
  const timers = new Map();
  const mediaListeners = [];
  let id = 0;
  const node = () => ({
    children: [], parentElement: null,
    get firstChild() { return this.children[0] || null; },
    get textContent() { return this.text || ''; },
    set textContent(value) { this.text = value; this.children = []; },
    id: '', className: '', hidden: false, dataset: {}, style: { setProperty() {}, removeProperty() {} },
    classList: { add() {}, remove() {} },
    setAttribute() {}, getAttribute() { return null; },
    replaceChildren(...items) { this.children = items; for (const item of items) item.parentElement = this; }, append(item) { this.children.push(item); item.parentElement = this; }, after() {}, closest() { return null; },
    getBoundingClientRect() { return { x: 0, y: 0, width: 80, height: 24 }; },
  });
  const doc = {
    hidden: false, body: { append() {} },
    createElement: node, querySelector: () => null, querySelectorAll: () => [], getElementById: () => null,
    addEventListener() {},
  };
  const media = { matches: false, addEventListener: (_k, fn) => mediaListeners.push(fn) };
  const ctx = vm.createContext({
    console, JSON, Map, Set, Promise, Date, queueMicrotask,
    document: doc, window: {}, state: { project: '/p/Study.magic', view: 'project' },
    localStorage: { getItem: () => null, setItem() {} },
    matchMedia: () => media,
    setTimeout: (fn, ms) => { timers.set(++id, { fn, at: ms }); return id; },
    clearTimeout: i => timers.delete(i),
  });
  vm.runInContext(read('companion-boxes.js'), ctx);
  ctx.el = (tag, attrs = {}, ...children) => {
    const element = doc.createElement(tag); Object.assign(element, attrs);
    element.replaceChildren(...children); return element;
  };
  vm.runInContext(read('companions.js'), ctx);
  return { ctx, boxes: ctx.window.companionBoxes, timers, media, mediaListeners };
}

const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
const tests = [
  ['submitted jobs stay busy and status follows their initiating panel', async () => {
    const { ctx, boxes } = fixture();
    boxes.register('render', {project:ctx.state.project});
    const start={box:'render',command:'blender.start',project:ctx.state.project,request:'render-1',params:{}};
    boxes.event({...start,phase:'start'});
    boxes.event({...start,phase:'success',result:{job:'job-1',state:'submitted'}});
    assert.equal(boxes.get('render').state,'working');
    assert.equal(boxes.get('render').job,'job-1');
    boxes.event({command:'blender.status',request:'independent-cli-poll',project:ctx.state.project,params:{job:'job-1'},phase:'success',result:{state:'failed',error:'Worker failed'}});
    assert.equal(boxes.get('render').state,'failed');
    assert.match(boxes.get('render').reason,/Worker failed/);
    boxes.event({...start,request:'render-2',phase:'start'});
    boxes.event({...start,request:'render-2',phase:'success',result:{job:'job-2',state:'submitted'}});
    boxes.event({command:'blender.status',project:ctx.state.project,params:{job:'job-1'},phase:'start'});
    assert.equal(boxes.get('render').job,'job-2','old poll start cannot replace the active job');
    boxes.event({command:'blender.status',project:ctx.state.project,params:{job:'job-1'},phase:'success',result:{state:'completed'}});
    assert.equal(boxes.get('render').state,'working','old job cannot complete the replacement');
    boxes.event({command:'blender.status',project:ctx.state.project,params:{job:'job-2'},phase:'success',result:{state:'completed'}});
    assert.equal(boxes.get('render').state,'ready');
  }],
  ['attention cannot inherit another project failure or receipt', () => {
    const {ctx,boxes}=fixture();
    boxes.fact({id:'chat',project:ctx.state.project,state:'failed',reason:'Old failure',request:'old-request',job:'old-job',context:{scene:'A'},resolution:'old resolution'});
    boxes.recordReceipt('chat',{revision:4,command:'old-command'});
    ctx.state.project='/p/Other.magic';
    boxes.attention('chat','Inspect this project');
    const box=boxes.get('chat');
    assert.equal(box.project,ctx.state.project);assert.equal(box.state,'idle');
    assert.equal(box.request,null);assert.equal(box.job,null);assert.equal(box.receipt,null);assert.equal(box.context,null);assert.equal(box.resolution,null);
    assert.equal(boxes.panels().items.find(row=>row.id==='chat').signal,2);
  }],
  ['reduced motion and disabling cancel outstanding pose animations', async () => {
    const { ctx, boxes, timers, media, mediaListeners } = fixture();
    const pets = ctx.createCompanions({frames:{}}, {boxes,sprites:{has:()=>true,image:()=>null}});
    boxes.fact({id:'problem',state:'failed'}); await flush();
    assert.ok(timers.size>0);
    media.matches=true; mediaListeners.forEach(fn=>fn());
    assert.equal(timers.size,0);
    media.matches=false; mediaListeners.forEach(fn=>fn());
    boxes.fact({id:'other',state:'failed'}); await flush();
    pets.setEnabled(false);
    assert.equal(timers.size,0);
    assert.ok(Object.values(pets.nodes).every(node=>node.hidden));
  }],
  ['explicit TUI attention wins over routine next-step readiness', async () => {
    const { ctx, boxes } = fixture();
    boxes.fact({id:'song',state:'needs-attention',priority:100,source:'readiness'});
    boxes.register('chat');
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    boxes.attention('chat', 'Inspect this result');
    await flush();
    assert.equal(pets.snapshot().sproket.box, 'chat');
    assert.equal(boxes.panels().items.find(row=>row.id==='chat').attention,'Inspect this result');
    boxes.attention('chat', null); await flush();
    assert.equal(pets.snapshot().sproket.box, 'song');
  }],
  ['pets see errors after the first sixty-four registered states', async () => {
    const { ctx, boxes } = fixture();
    for (let i = 0; i < 70; i++) boxes.register(`panel-${i}`);
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    boxes.fact({ id: 'panel-69', state: 'failed', reason: 'Render failed' });
    await flush();
    assert.equal(pets.snapshot().sproket.box, 'panel-69');
    assert.equal(pets.snapshot().sproket.signal, 3);
  }],
  ['rails require a configured destination and never fall back elsewhere', () => {
    const { ctx } = fixture();
    const rail = { getAttribute: key => key === 'data-companion-rail' ? 'panel' : null, getBoundingClientRect: () => ({ width: 100, height: 40 }) };
    const slot = { getAttribute: key => key === 'data-companion-slot' ? 'panel' : null, closest: () => rail, getBoundingClientRect: () => ({ left: 12, top: 8, width: 64, height: 28 }) };
    ctx.document.querySelectorAll = selector => selector === '[data-companion-rail]' ? [rail] : selector === '[data-companion-slot]' ? [slot] : [];
    ctx.window.uiConfig = { rails: { panel: { accept: 'yes' } }, slots: { panel: { target: 'panel' } } };
    vm.runInContext(read('companion-rails.js'), ctx);
    const rails = ctx.createCompanionRails(); rails.discover();
    assert.equal(rails.resolve({ name: 'sproket', target: 'missing' }), null);
    assert.equal(rails.resolve({ name: 'sproket', target: 'panel' }).slotId, 'panel');
    assert.equal(rails.measure('panel').x, 12);
    slot.getBoundingClientRect = () => ({left: 12, top: 8, width: 0, height: 28});
    rails.invalidate();
    assert.equal(rails.measure('panel'), null, 'zero-width slots cannot host a pet');
    ctx.innerWidth = 200; ctx.innerHeight = 100;
    slot.getBoundingClientRect = () => ({left: 12, top: 90, width: 64, height: 28});
    rails.invalidate();
    assert.equal(rails.resolve({ name: 'sproket', target: 'panel' }), null, 'partially offscreen slots cannot host a pet');
    slot.getBoundingClientRect = () => ({left: 12, top: 8, width: 64, height: 28});
    rails.invalidate();
    assert.equal(rails.resolve({ name: 'sproket', target: 'panel' }).slotId, 'panel');
    delete ctx.window.uiConfig.slots.panel; rails.discover();
    assert.equal(rails.resolve({ name: 'sproket', target: 'panel' }), null);
  }],
  ['UI and bridge results update the same box, ignoring stale project events', async () => {
    const { ctx, boxes } = fixture();
    const owner = { getAttribute: () => 'task' };
    const control = { getAttribute: () => 'fixture.run', closest: () => owner };
    ctx.document.querySelectorAll = selector => selector === '[data-commands]' ? [control] : [];
    boxes.register('task', { project: ctx.state.project });
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    const event = { ...boxes.capture('fixture.run', { _request_id: 'first' }), phase: 'start' };
    boxes.event(event); await flush();
    assert.equal(boxes.get('task').state, 'working');
    boxes.event({ ...event, phase: 'error', error: 'Actual failure' }); await flush();
    assert.equal(pets.snapshot().sproket.box, 'task');
    boxes.event({ command: 'fixture.run', project: ctx.state.project, request: 'second', phase: 'success', result: { ok: true } }); await flush();
    assert.equal(boxes.get('task').state, 'ready');
    assert.equal(pets.snapshot().sproket.box, null);
    boxes.event({ ...event, project: '/other.magic', phase: 'error' });
    assert.equal(boxes.get('task').state, 'ready');
  }],
  ['sprite stays attached after drawing and updates', async () => {
    const { ctx, boxes } = fixture();
    const sprites = { has: () => true, image: () => ctx.document.createElement('img') };
    const pets = ctx.createCompanions({ frames: {} }, { boxes, sprites });
    pets.setEnabled(true);
    for (const state of ['working', 'ready', 'failed']) {
      boxes.fact({ id: 'work', project: '/p/Study.magic', state });
      await flush();
      for (const node of Object.values(pets.nodes)) assert.ok(node.firstChild?.firstChild, 'sprite image must stay in its holder');
    }
  }],
  ['working is visibly different from completed work', async () => {
    const { ctx, boxes } = fixture();
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    boxes.fact({ id: 'work', project: '/p/Study.magic', state: 'working' });
    await flush();
    assert.equal(pets.snapshot().gaffer.pose, 'work');
    boxes.fact({ id: 'work', project: '/p/Study.magic', state: 'ready' });
    await flush();
    assert.equal(pets.snapshot().gaffer.pose, 'rest');
  }],
  ['one number per state, and the light cannot disagree with it', () => {
    const { boxes } = fixture();
    const expect = { idle: 0, ready: 1, working: 1, 'needs-attention': 2, failed: 3 };
    for (const [state, value] of Object.entries(expect)) {
      assert.equal(boxes.signal(state), value, `${state} signal`);
      const dot = boxes.dot(state);
      assert.equal(dot.dataset.signal, String(value), `${state} dot attr`);
      assert.equal(dot.className, 'dot', `${state} dot class`);
    }
  }],
  ['readers are notified once per change batch, never per fact', async () => {
    const { boxes } = fixture();
    let calls = 0;
    boxes.subscribe(() => calls++);
    for (let i = 0; i < 5; i++) boxes.fact({ id: `b${i}`, project: '/p/Study.magic', state: 'working' });
    await flush();
    assert.equal(calls, 1, 'five facts must coalesce into one notification');
    boxes.fact({ id: 'b1', project: '/p/Study.magic', state: 'failed' });
    await flush();
    assert.equal(calls, 2);
  }],
  ['Sproket takes the worst signal, Gaffer the newest', async () => {
    const { ctx, boxes } = fixture();
    boxes.fact({ id: 'green', project: '/p/Study.magic', state: 'ready' });
    boxes.fact({ id: 'red', project: '/p/Study.magic', state: 'failed' });
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    await flush();
    const snap = pets.snapshot();
    assert.equal(snap.sproket.box, 'red');
    assert.equal(snap.sproket.signal, 3);
    assert.equal(snap.sproket.pose, 'alert');
    assert.equal(snap.gaffer.box, 'red', 'newest change wins for the cat');
  }],
  ['a light drawn for a panel is that panel\'s signal', async () => {
    const { ctx, boxes } = fixture();
    const light = boxes.dot('needs-attention', '', 'block-song');
    assert.equal(light.dataset.signal, '2');
    assert.equal(boxes.panels().items.find(item => item.id === 'block-song').signal, 2, 'the record says what the screen shows');
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    await flush();
    assert.equal(pets.snapshot().sproket.box, 'block-song', 'the dog follows the yellow light');
    boxes.dot('ready', '', 'block-song');
    await flush();
    assert.equal(pets.snapshot().sproket.box, null, 'and leaves when it turns green');
  }],
  ['each pet finishes its own burst', async () => {
    const { ctx, boxes, timers } = fixture();
    const sprites = { has: () => true, image: () => null };
    boxes.fact({ id: 'red', project: '/p/Study.magic', state: 'failed' });
    const pets = ctx.createCompanions({ frames: {} }, { boxes, sprites });
    pets.setEnabled(true);
    await flush();
    assert.equal(timers.size, 2, 'the dog and the cat each start a burst');
    for (let round = 0; round < 10 && timers.size; round++) for (const [key, timer] of [...timers]) { timers.delete(key); timer.fn(); }
    const snap = pets.snapshot();
    assert.equal(snap.sproket.pose, 'alert', 'the dog settles on the red light, not frozen mid-burst');
    assert.equal(snap.gaffer.pose, 'work', 'the cat settles at work on the newest change');
  }],
  ['news within the cooldown makes one burst, not two', async () => {
    const { ctx, boxes, timers } = fixture();
    const sprites = { has: () => true, image: () => null };
    const pets = ctx.createCompanions({ frames: {} }, { boxes, sprites });
    pets.setEnabled(true);
    boxes.fact({ id: 'first', project: '/p/Study.magic', state: 'failed' });
    await flush();
    const started = timers.size;
    boxes.fact({ id: 'second', project: '/p/Study.magic', state: 'failed' });
    await flush();
    assert.equal(timers.size, started, 'no second burst inside the cooldown');
  }],
  ['ASCII pets still draw without a sprite', async () => {
    const { ctx, boxes } = fixture();
    boxes.fact({ id: 'green', project: '/p/Study.magic', state: 'ready' });
    const pets = ctx.createCompanions({ frames: { sproket: ['dog'], gaffer: ['cat'] } }, { boxes });
    pets.setEnabled(true);
    await flush();
    assert.equal(pets.nodes.sproket.textContent, 'dog');
    assert.equal(pets.nodes.gaffer.textContent, 'cat');
    assert.equal(pets.nodes.sproket.hidden, true, 'ASCII cannot bypass missing configured rails');
    assert.match(pets.snapshot().sproket.placement_error, /configured visible rail/);
  }],
  ['yellow is attention, green is only in use', async () => {
    const { ctx, boxes } = fixture();
    boxes.fact({ id: 'yellow', project: '/p/Study.magic', state: 'needs-attention' });
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    await flush();
    assert.equal(pets.snapshot().sproket.pose, 'work');
    boxes.fact({ id: 'yellow', project: '/p/Study.magic', state: 'ready' });
    await flush();
    const snap = pets.snapshot();
    assert.equal(snap.sproket.box, null, 'a green box is not a warning');
    assert.equal(snap.gaffer.box, 'yellow', 'the cat still follows the work');
    assert.equal(snap.gaffer.pose, 'rest');
  }],
  ['commands and jobs move nothing; only box signals do', async () => {
    const { ctx, boxes } = fixture();
    const pets = ctx.createCompanions({ frames: {} }, { boxes });
    pets.setEnabled(true);
    const before = JSON.stringify(pets.snapshot().sproket);
    boxes.event({ command: 'blender.start', project: '/p/Study.magic', phase: 'error' });
    boxes.event({ command: 'agent.send', project: '/p/Study.magic', phase: 'error' });
    await flush();
    assert.equal(JSON.stringify(pets.snapshot().sproket), before);
  }],
  ['a command receipt rides on the control, never in per-element save logic', async () => {
    const { boxes } = fixture();
    boxes.register('save', { project: '/p/Study.magic' });
    const receipt = boxes.recordReceipt('save', { undo: 'tok-1', revision: 7, dirty: false, command: 'project.save' });
    assert.equal(receipt.undo, 'tok-1', 'undo token is kept');
    assert.equal(receipt.revision, 7, 'project revision is kept');
    assert.equal(receipt.saved, true, 'a clean result is saved');
    const row = boxes.snapshot({ project: '/p/Study.magic' }).find(r => r.id === 'save');
    assert.deepEqual(row.receipt, receipt, 'readers see the same receipt');
    assert.equal(boxes.recordReceipt('save', { dirty: true }).saved, false);
  }],
  ['registry keeps boxes across project switch', () => {
    const { ctx, boxes } = fixture();
    boxes.register('block-song', { project: '/p/A.magic' });
    ctx.state.project = '/p/B.magic';
    // re-render registers readiness again, without an explicit project
    boxes.readiness('block-song', { state: 'ready', reason: 'Ready' });
    const ids = boxes.snapshot({ project: '/p/B.magic' }).map(r => r.id);
    assert.ok(ids.includes('block-song'), `expected block-song in B snapshot, got ${ids}`);
  }],
  ['no engine command id may live in the companion controller', () => {
    const source = read('companions.js');
    const hit = source.match(/\b(agent|image|blender|storyboard|mlt|synfig|opentoonz|video|sprocket)\.[a-z_]+/g);
    assert.equal(hit, null, `companion controller must not know commands: ${hit}`);
  }],
];

let failed = 0;
for (const [name, test] of tests) {
  try { await test(); console.log('PASS companions: ' + name); }
  catch (e) { failed++; console.error('FAIL companions: ' + name + '\n' + e.stack); }
}
assert.equal(failed, 0, `${failed} companion regressions failed`);
