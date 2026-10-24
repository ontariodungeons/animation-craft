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
    id: '', className: '', hidden: false, textContent: '', dataset: {}, style: {},
    classList: { add() {}, remove() {} },
    setAttribute() {}, getAttribute() { return null; },
    replaceChildren() {}, append() {}, after() {}, closest() { return null; },
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
  vm.runInContext(read('companions.js'), ctx);
  return { ctx, boxes: ctx.window.companionBoxes, timers };
}

const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
const tests = [
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
    assert.equal(snap.sproket.pose, 'rest', 'the dog is not frozen mid-burst');
    assert.equal(snap.gaffer.pose, 'rest');
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
    pets.event({ command: 'blender.start', project: '/p/Study.magic', phase: 'error' });
    pets.event({ command: 'agent.send', project: '/p/Study.magic', phase: 'error' });
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
