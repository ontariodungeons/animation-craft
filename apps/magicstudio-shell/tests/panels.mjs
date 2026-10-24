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
  const el = {
    tagName: tag.toUpperCase(),
    id,
    title,
    role,
    textContent: text,
    className: '',
    dataset: {},
    parent,
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
    getElementById: () => null,
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
  vm.runInContext(read('companion-boxes.js'), ctx);
  vm.runInContext(read('bridge.js'), ctx);
  return { ctx, boxes: ctx.companionBoxes, bridge: ctx.acBridge, elements };
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
    assert.deepEqual(plain(report.items[0]), { id: 'block-song', label: 'Song', signal: 0, open: false, busy: false, dirty: false });
    assert.deepEqual(Object.keys(report.items[0]), ['id', 'label', 'signal', 'open', 'busy', 'dirty']);
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
];

let failed = 0;
for (const [name, test] of tests) {
  try { await test(); console.log('PASS panels: ' + name); }
  catch (e) { failed++; console.error('FAIL panels: ' + name + '\n' + e.stack); }
}
assert.equal(failed, 0, `${failed} panel regressions failed`);
