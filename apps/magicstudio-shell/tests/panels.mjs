// One standard record per panel: flags, a revision that changes only when a
// flag changes, and the clickable controls that belong to no known panel.
// Loads the real companion-boxes.js and bridge.js in a vm with a stub DOM.
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = name => readFileSync(new URL(`../ui/${name}`, import.meta.url), 'utf8');
// The vm has its own realm: compare plain data, not cross-realm objects.
const plain = value => JSON.parse(JSON.stringify(value));

function element(tag, { id = '', title = '', text = '', attrs = {}, parent = null, role = '' } = {}) {
  const attributes = { ...attrs };
  if (id) attributes.id = id;
  const el = {
    tagName: tag.toUpperCase(),
    id,
    title,
    role,
    textContent: text,
    className: '',
    dataset: {},
    style: {},
    parent,
    parentElement: parent,
    getAttribute(name) { return Object.prototype.hasOwnProperty.call(attributes, name) ? attributes[name] : null; },
    setAttribute(name, value) { attributes[name] = String(value); },
    matches(selector) {
      for (const part of String(selector).split(',').map(s => s.trim())) {
        if (!part) continue;
        if (part === "*") return true;
        if (part.startsWith('[')) {
          const m = part.match(/^\[([^\]=]+)(?:="([^"]*)")?\]$/);
          if (!m) continue;
          const value = el.getAttribute(m[1]);
          if (value === null) continue;
          if (m[2] === undefined || value === m[2]) return true;
          continue;
        }
        if (part.startsWith('.')) { if (el.className.split(/\s+/).includes(part.slice(1))) return true; continue; }
        const tag = part.split('[')[0];
        if (tag && el.tagName.toLowerCase() === tag.toLowerCase() && (!part.includes('[') || el.matches(part.slice(part.indexOf('['))))) return true;
      }
      return false;
    },
    closest(selector) {
      let node = el;
      while (node) {
        if (node.matches(selector)) return node;
        node = node.parent;
      }
      return null;
    },
    getBoundingClientRect() { return { x: 0, y: 0, left: 0, top: 0, width: 120, height: 24 }; },
    contains(node) { for (let n = node; n; n = n.parentElement) if (n === el) return true; return false; },
  };
  return el;
}

function fixture() {
  const elements = [];
  const doc = {
    hidden: false,
    body: { append() {} },
    createElement: () => element('span'),
    querySelector: () => null,
    querySelectorAll: (selector) => elements.filter(el => el.matches(selector)),
    getElementById: (id) => elements.find(el => el.id === id) || null,
    addEventListener() {},
  };
  doc.body.querySelectorAll = doc.querySelectorAll;
  const ctx = vm.createContext({
    console, JSON, Map, Set, Promise, Date, queueMicrotask, Event: class { constructor(type) { this.type = type; } },
    document: doc,
    state: { project: '/p/Study.magic', view: 'project' },
    localStorage: { getItem: () => null, setItem() {} },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    setTimeout: (fn) => fn(),
    clearTimeout() {},
    getComputedStyle: () => ({ visibility: 'visible' }),
    innerWidth: 1200,
    innerHeight: 800,
  });
  vm.runInContext('globalThis.window = globalThis;', ctx);
  ctx.uiConfig = { version: 1, looks: {}, densities: [], elements: {} };
  vm.runInContext(read('companion-boxes.js'), ctx);
  vm.runInContext(read('bridge.js'), ctx);
  return { ctx, boxes: ctx.companionBoxes, bridge: ctx.magicBridge, elements };
}

// el() and the config reader, as the page runs them.
function elFixture(entries) {
  const created = [];
  const node = (tag) => {
    const element = { tagName: tag.toUpperCase(), className: '', dataset: {}, attributes: {}, children: [], disabled: false, selected: false, checked: false,
      setAttribute(name, value) { this.attributes[name] = String(value); },
      getAttribute(name) { return Object.prototype.hasOwnProperty.call(this.attributes, name) ? this.attributes[name] : null; },
      append(...items) { this.children.push(...items); for (const item of items) if (typeof item === 'object') item.parentElement = this; },
      querySelector(selector) {
        const all = this.children.flatMap(child => typeof child === 'object' ? [child, ...(child.children || [])] : []);
        return all.find(child => selector === '.panel-head' ? child.className?.split(' ').includes('panel-head') : child.getAttribute?.('data-companion-slot') !== null) || null;
      }, addEventListener() {} };
    created.push(element);
    return element;
  };
  const doc = { createElement: node, getElementById: () => null, querySelector: () => null, querySelectorAll: () => [], addEventListener() {}, body: { querySelectorAll: () => [], classList: { contains: () => false, add() {}, remove() {} } } };
  const ctx = vm.createContext({ console, JSON, Promise, document: doc, state: { project: '/p/Study.magic' },
    fetch: async () => ({ ok: true, json: async () => ({ version: 1, looks: {}, densities: [], elements: entries }) }),
    queueMicrotask, window: {} });
  vm.runInContext('globalThis.window = globalThis;', ctx);
  const source = readFileSync(new URL('../ui/app.js', import.meta.url), 'utf8');
  vm.runInContext(source.slice(source.indexOf('const uiConfig = {'), source.indexOf('const kindOf =')), ctx);
  return { ctx, created };
}

const tests = [
  ['shared controls preserve false HTML boolean properties and ARIA text', () => {
    const { ctx } = elFixture({});
    const control = vm.runInContext('el("button", {disabled:false, "aria-expanded":false})', ctx);
    assert.equal(control.disabled, false);
    assert.equal(control.getAttribute('disabled'), null);
    assert.equal(control.getAttribute('aria-expanded'), 'false');
    assert.equal(vm.runInContext('el("button", {disabled:true})', ctx).disabled, true);
    assert.equal(vm.runInContext('el("option", {selected:false})', ctx).selected, false);
    assert.equal(vm.runInContext('el("option", {selected:true})', ctx).selected, true);
    assert.equal(vm.runInContext('el("input", {checked:false})', ctx).checked, false);
  }],
  ['missing shared panel header is a reported contract failure', () => {
    const { ctx, bridge, boxes, elements } = fixture();
    ctx.uiConfig.elements.lyrics = { component: 'panel', indicator: 'hidden' };
    boxes.register('lyrics', { project: '/p/Study.magic' });
    const node = element('section', { id: 'lyrics', attrs: { 'data-box-id': 'lyrics', 'data-companion-rail': 'lyrics' } });
    node.dataset.contractError = 'Panel has no shared header'; elements.push(node);
    assert.equal(bridge.unconfigured()[0].reason, 'missing-panel-header');
    assert.equal(boxes.panels().items.find(row => row.id === 'lyrics').signal, 3);
  }],
  ['missing state inside a configured panel is a red contract error', () => {
    const { ctx, bridge, boxes, elements } = fixture();
    ctx.uiConfig.elements.lyrics = { component: 'panel', indicator: 'hidden' };
    boxes.register('lyrics', { project: '/p/Study.magic' });
    const parent = element('section', { id: 'lyrics', attrs: { 'data-box-id': 'lyrics', 'data-companion-rail': 'lyrics' } });
    const broken = element('button', { id: 'new-control', parent, attrs: { 'data-box-id': 'missing-state' } });
    elements.push(parent, broken);
    assert.ok(bridge.unconfigured().some(row => row.id === 'new-control'));
    const fault = boxes.panels().items.find(row => row.id === 'lyrics');
    assert.equal(fault.signal, 3, 'optional visibility must not conceal missing state');
    assert.match(fault.reason, /UI contract/);
    boxes.fact({ id: 'lyrics', state: 'failed', reason: 'Actual lyric load failed' });
    boxes.register('missing-state', { project: '/p/Study.magic' });
    const repaired = boxes.panels().items.find(row => row.id === 'lyrics');
    assert.equal(repaired.signal, 3, 'repair must preserve an independent engine error');
    assert.match(repaired.reason, /Actual lyric load failed/);
    boxes.fact({ id: 'lyrics', state: 'idle' });
    assert.equal(boxes.panels().items.find(row => row.id === 'lyrics').signal, 0, 'healthy state clears after the real error is resolved');
  }],
  ['one shared panel constructor mounts the configured rail exactly once', async () => {
    const { ctx } = elFixture({ panel: { component: 'panel', look: 'chat', rail: 'panel' } });
    await ctx.loadComponents();
    const head = ctx.el('div', { class: 'panel-head' });
    const panel = ctx.el('section', { id: 'panel' }, head);
    ctx.applyEntry(panel, ctx.uiConfig.elements.panel, 'panel');
    assert.equal(panel.getAttribute('data-companion-rail'), 'panel');
    assert.equal(head.children.length, 1);
    assert.equal(head.children[0].getAttribute('data-companion-slot'), 'panel');
    assert.equal(panel.dataset.uiConfig, 'panel');
  }],
  ['CLI toggles update checked state and indexed actions require instance', () => {
    const { bridge, elements } = fixture();
    const control = element('input', { id: 'toggle' });
    control.type = 'checkbox'; control.checked = false; control.dispatchEvent = () => {};
    elements.push(control);
    bridge.set('toggle', true);
    assert.equal(control.checked, true);
    assert.throws(() => bridge.set('toggle', 'true'), /boolean/);
    assert.throws(() => bridge.click('@1'), /instance/);
    control.disabled = true;
    assert.throws(() => bridge.set('toggle', false), /not editable/);
  }],
  ['indices and positions survive reads; changed snapshots reject old pagination', () => {
    const { bridge, elements } = fixture();
    const parent = element('div', { id: 'panel' }), child = element('button', { parent });
    elements.push(parent, child);
    const first = bridge.controls({});
    const row = first.items[1];
    assert.ok(Number.isInteger(row.index));
    assert.equal(row.parent_index, first.items[0].index);
    assert.deepEqual(plain(row.rect), [0, 0, 120, 24]);
    assert.equal(row.target, `@${row.index}`);
    assert.equal(bridge.controls({}).items[1].index, row.index);
    assert.throws(() => bridge.controls({ expected_instance: 'another-page' }), /instance changed/);
    child.disabled = true;
    assert.throws(() => bridge.controls({ expected_revision: first.revision }), /snapshot changed/);
  }],
  ['control telemetry pages all controls without returning input contents', () => {
    const { bridge, elements } = fixture();
    for (let i = 0; i < 70; i++) {
      const control = element('input', { id: `field-${i}` });
      control.value = 'private draft';
      elements.push(control);
    }
    const first = bridge.controls({});
    assert.equal(first.total, 70);
    assert.equal(first.items.length, 64);
    assert.equal(first.next_offset, 64);
    const last = bridge.controls({ offset: first.next_offset });
    assert.equal(last.items.length, 6);
    assert.equal(last.next_offset, null);
    assert.deepEqual(plain(first.items[0].actions), ['ui.set', 'ui.key']);
    assert.ok(!JSON.stringify(first).includes('private draft'));
  }],
  ['attention is project-bound and state reports include every panel', () => {
    const { bridge, boxes } = fixture();
    for (let i = 0; i < 70; i++) boxes.register(`panel-${i}`);
    assert.throws(() => bridge.attention({ project: '/wrong.magic', id: 'panel-0', reason: 'Next' }), /does not match/);
    const reply = bridge.attention({ project: '/p/Study.magic', id: 'panel-0', reason: 'Next' });
    assert.equal(reply.panels.total_items, 70);
    assert.equal(reply.panels.truncated, false);
    assert.equal(reply.panels.items.length, 70);
    boxes.fact({id: "panel-69", state: "failed", reason: "Last panel failed"});
    assert.equal(boxes.panels().items.find(p => p.id === "panel-69").signal, 3);
    assert.equal(reply.panels.items[0].signal, 2);
    bridge.attention({ project: '/p/Study.magic', id: 'panel-0', clear: true });
    assert.equal(boxes.get('panel-0').attention, null);
  }],
  ['hidden indicator still reports attention and preserves errors', () => {
    const { ctx, boxes } = fixture();
    ctx.window.uiConfig = { elements: { quiet: { indicator: 'hidden' } } };
    const dot = boxes.dot('idle', '', 'quiet');
    assert.equal(dot.hidden, true);
    assert.equal(dot.textContent, '💤');
    boxes.attention('quiet', 'Check this next');
    assert.equal(boxes.panels().items[0].signal, 2);
    boxes.fact({ id: 'quiet', state: 'failed', reason: 'Render failed' });
    boxes.attention('quiet', null);
    assert.equal(boxes.panels().items[0].signal, 3);
    assert.throws(() => boxes.attention('missing', 'Look here'), /Unknown panel/);
  }],
  ['panels() shape matches contract', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const report = boxes.panels();
    assert.equal(report.available, true);
    assert.equal(typeof report.revision, 'number');
    assert.ok(Array.isArray(report.items));
    assert.ok(Array.isArray(report.unconfigured));
    assert.deepEqual(plain(report.items[0]), { id: 'block-song', label: 'Song', look: null, signal: 0, attention: null, busy: false, reason: null, source: null, request: null, project: null, context: null, state: 'idle', seq: 0, priority: 0, job: null, resolution: null, observed_at: null, receipt: null });
  }],
  ['revision unchanged when nothing changed', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const first = boxes.panels().revision;
    assert.equal(boxes.panels().revision, first);
    boxes.fact({ id: 'block-song', state: 'ready' });
    const afterFact = boxes.panels().revision;
    boxes.fact({ id: 'block-song', state: 'ready' });
    assert.equal(boxes.panels().revision, afterFact, 'the same fact again changes nothing');
  }],
  ['revision bumps once per real change', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const before = boxes.panels().revision;
    boxes.fact({ id: 'block-song', state: 'working' });
    const after = boxes.panels().revision;
    assert.equal(after, before + 1);
    boxes.fact({ id: 'block-song', state: 'working' });
    assert.equal(boxes.panels().revision, after);
    boxes.fact({ id: 'block-song', state: 'needs-attention' });
    assert.equal(boxes.panels().revision, after + 1);
  }],
  ['el() applies data-look and data-density from the entry', async () => {
    const { ctx } = elFixture({ 'block-song': { component: 'panel-head', look: 'control', density: 'normal', label: 'Song' } });
    await ctx.loadComponents();
    const node = ctx.el('button', { id: 'block-song' });
    assert.equal(node.className, 'panel-head', 'the component class comes from the entry without caller styling');
    const active = ctx.el('button', { id: 'block-song', class: 'active panel-head' });
    assert.equal(active.className, 'active panel-head', 'behavioral classes survive and the component is not duplicated');
    assert.equal(node.dataset.look, 'control', 'the look comes from the entry');
    assert.equal(node.dataset.density, 'normal', 'the density comes from the entry');
    const plain = ctx.el('button', { id: 'not-configured' });
    assert.equal(plain.dataset.look, undefined, 'an element without an entry gets no look');
  }],
  ['every components.json look is in looks and every density is in densities', () => {
    const config = JSON.parse(readFileSync(new URL('../ui/components.json', import.meta.url), 'utf8'));
    const looks = config.looks ?? {};
    const densities = config.densities ?? [];
    for (const [id, entry] of Object.entries(config.elements ?? {})) {
      if (entry.look !== undefined) {
        const allowed = looks[entry.component] ?? [];
        assert.ok(allowed.includes(entry.look), `${id}: look ${entry.look} is not one of ${allowed.join(', ') || 'none'}`);
      }
      if (entry.density !== undefined) assert.ok(densities.includes(entry.density), `${id}: density ${entry.density} is not one of ${densities.join(', ')}`);
    }
  }],
  ['look is reported in panels()', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song', look: 'control', density: 'normal' });
    assert.equal(boxes.panels().items[0].look, 'control');
  }],
  ['busy follows the working state', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    assert.equal(boxes.panels().items[0].busy, false);
    boxes.fact({ id: 'block-song', state: 'working' });
    const item = boxes.panels().items[0];
    assert.equal(item.busy, true, 'work in flight is busy');
    assert.equal(item.signal, 1, 'work in flight is in use');
  }],
  ['element without an entry is untracked', () => {
    const { boxes, bridge, elements } = fixture();
    const stray = element('button', { id: 'stray-1', text: 'Stray' });
    elements.push(stray);
    assert.equal(bridge.unconfigured().find(row => row.id === 'stray-1').reason, 'untracked');
    boxes.register('known', { label: 'Known' });
    const inside = element('button', { id: 'known-1', text: 'Inside' });
    const host = element('section', { attrs: { 'data-box-id': 'known' } });
    inside.parent = host;
    inside.parentElement = host;
    elements.push(host, inside);
    assert.equal(bridge.unconfigured().some(row => row.id === 'known-1'), false);
  }],
  ['visual inline style is unstyled', () => {
    const { boxes, bridge, elements } = fixture();
    boxes.register('known', { label: 'Known' });
    const styled = element('div', { id: 'styled-1', text: 'Styled' });
    styled.style = { left: '10px' };
    const host = element('section', { attrs: { 'data-box-id': 'known' } });
    styled.parent = host;
    styled.parentElement = host;
    elements.push(host, styled);
    assert.equal(bridge.unconfigured().find(row => row.id === 'styled-1').reason, 'unstyled');
  }],
  ['geometry custom properties are not unstyled', () => {
    const { boxes, bridge, elements } = fixture();
    boxes.register('known', { label: 'Known' });
    const placed = element('div', { id: 'placed-1', text: 'Placed' });
    placed.style = { '--x': '10px', '--w': '20px' };
    const host = element('section', { attrs: { 'data-box-id': 'known' } });
    placed.parent = host;
    placed.parentElement = host;
    elements.push(host, placed);
    assert.equal(bridge.unconfigured().some(row => row.id === 'placed-1'), false);
  }],
  ['unconfigured lists clickable without data-box-id ancestor', () => {
    const { boxes, elements } = fixture();
    const panel = element('section', { attrs: { 'data-box-id': 'block-song' } });
    const inside = element('button', { text: 'Export', parent: panel });
    const outside = element('button', { text: 'Save', parent: null });
    elements.push(panel, inside, outside);
    boxes.register('block-song', { label: 'Song' });
    const unconfigured = boxes.panels().unconfigured;
    assert.equal(unconfigured.length, 1);
    assert.equal(unconfigured[0].tag, 'button');
    assert.equal(unconfigured[0].label, 'Save');
    assert.equal(unconfigured[0].reason, 'untracked');
    assert.ok(unconfigured[0].id);
    assert.ok(!unconfigured.some(row => row.label === 'Export'));
  }],
  ['child of registered panel is configured', () => {
    const { boxes, elements } = fixture();
    const panel = element('section', { attrs: { 'data-box-id': 'block-song' } });
    const inside = element('button', { text: 'Export', parent: panel });
    elements.push(panel, inside);
    boxes.register('block-song', { label: 'Song' });
    assert.equal(boxes.panels().unconfigured.length, 0);
    assert.equal(boxes.panels().items[0].id, 'block-song');
  }],
  ['panels() reports only the current project', () => {
    const { boxes } = fixture();
    boxes.register('old-panel', { label: 'Old', project: '/p/Old.magic' });
    boxes.register('block-song', { label: 'Song', project: '/p/Study.magic' });
    boxes.register('shared', { label: 'Shared' });
    assert.deepEqual(plain(boxes.panels().items.map(item => item.id)), ['block-song', 'shared']);
  }],
];

let failed = 0;
for (const [name, test] of tests) {
  try { await test(); console.log('PASS panels: ' + name); }
  catch (e) { failed++; console.error('FAIL panels: ' + name + '\n' + e.stack); }
}
assert.equal(failed, 0, `${failed} panel regressions failed`);

// Drawing a real timeline curve must use the same registered tones as its row.
{
 const draw={setTransform(){},beginPath(){},roundRect(){},fill(){},moveTo(){},lineTo(){},stroke(){},fillText(){}};
 const canvas={clientWidth:100,clientHeight:30,getContext(){return draw;}};
 const context=vm.createContext({uiConfig:{timeline_tones:{rotation:'--pose-purple'}},document:{documentElement:{}},devicePixelRatio:1,
  ResizeObserver:class{},getComputedStyle:()=>({getPropertyValue:key=>{assert.ok(['--pose-purple','--depth-gold'].includes(key));return '#c3a4ff';}})});
 vm.runInContext(read('timeline.js'),context);
 context.canvas=canvas;
 vm.runInContext('timelineDraw(canvas,{kind:"rotation",values:[0,1,0]},[],1)',context);
 console.log('PASS timeline: curve colour uses the shared config');
}
