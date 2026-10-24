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
        if (tag && el.tagName.toLowerCase() === tag.toLowerCase()) return true;
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
  const ctx = vm.createContext({
    console, JSON, Map, Set, Promise, Date, queueMicrotask,
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

const tests = [
  ['panels() shape matches contract', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const report = boxes.panels();
    assert.equal(report.available, true);
    assert.equal(typeof report.revision, 'number');
    assert.ok(Array.isArray(report.items));
    assert.ok(Array.isArray(report.unconfigured));
    assert.deepEqual(plain(report.items[0]), { id: 'block-song', label: 'Song', look: null, signal: 0, open: false, busy: false, dirty: false });
    assert.deepEqual(Object.keys(report.items[0]), ['id', 'label', 'look', 'signal', 'open', 'busy', 'dirty']);
  }],
  ['revision unchanged when nothing changed', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const first = boxes.panels().revision;
    assert.equal(boxes.panels().revision, first);
    boxes.flag('block-song', 'open', false);
    assert.equal(boxes.panels().revision, first);
  }],
  ['revision bumps once per real change', () => {
    const { boxes } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const before = boxes.panels().revision;
    boxes.flag('block-song', 'busy', true);
    const after = boxes.panels().revision;
    assert.equal(after, before + 1);
    boxes.flag('block-song', 'busy', true);
    assert.equal(boxes.panels().revision, after);
    boxes.flag('block-song', 'dirty', true);
    assert.equal(boxes.panels().revision, after + 1);
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
  ['open follows the panel element', () => {
    const { boxes, elements } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const panel = element('section', { attrs: { 'data-box-id': 'block-song' } });
    elements.push(panel);
    assert.equal(boxes.panels().items[0].open, true, 'a rendered panel is open');
    panel.setAttribute('aria-expanded', 'false');
    assert.equal(boxes.panels().items[0].open, false, 'a collapsed panel is not open');
    panel.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, width: 0, height: 0 });
    assert.equal(boxes.panels().items[0].open, false, 'a panel that is not rendered is not open');
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
  ['dirty follows a draft field inside the panel', () => {
    const { boxes, elements } = fixture();
    boxes.register('block-song', { label: 'Song' });
    const panel = element('section', { attrs: { 'data-box-id': 'block-song' } });
    const field = element('input', { id: 'field-1' });
    field.parent = panel;
    field.parentElement = panel;
    elements.push(panel, field);
    assert.equal(boxes.panels().items[0].dirty, false, 'a panel without a draft field is clean');
    boxes.drafts(['field-1']);
    assert.equal(boxes.panels().items[0].dirty, true, 'a draft field inside the panel makes it dirty');
    boxes.drafts(['field-elsewhere']);
    assert.equal(boxes.panels().items[0].dirty, false, 'a draft field elsewhere does not');
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
  ['panel() refuses flag with no handler', () => {
    const { boxes, bridge } = fixture();
    boxes.register('block-song', { label: 'Song' });
    assert.throws(() => bridge.panel('block-song', 'open', true), /^Error: panel block-song does not accept open$/);
    let seen = null;
    boxes.register('block-lyrics', { label: 'Lyrics', on: { busy: (value) => { seen = value; } } });
    bridge.panel('block-lyrics', 'busy', true);
    assert.equal(seen, true);
  }],
  ['panels() reports only the current project', () => {
    const { boxes } = fixture();
    boxes.register('old-panel', { label: 'Old', project: '/p/Old.magic' });
    boxes.register('block-song', { label: 'Song', project: '/p/Study.magic' });
    boxes.register('shared', { label: 'Shared' });
    assert.deepEqual(plain(boxes.panels().items.map(item => item.id)), ['block-song', 'shared']);
  }],
  ['a failing handler leaves the flag unchanged', () => {
    const { boxes, bridge } = fixture();
    boxes.register('block-song', { label: 'Song', on: { busy: () => { throw new Error('render refused'); } } });
    assert.throws(() => bridge.panel('block-song', 'busy', true), /render refused/);
    assert.equal(boxes.panels().items[0].busy, false);
  }],
];

let failed = 0;
for (const [name, test] of tests) {
  try { await test(); console.log('PASS panels: ' + name); }
  catch (e) { failed++; console.error('FAIL panels: ' + name + '\n' + e.stack); }
}
assert.equal(failed, 0, `${failed} panel regressions failed`);
