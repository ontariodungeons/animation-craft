// Read-only: sample when each HyperFrames clip is visible by running the composition's own
// per-frame script (`renderFishAt` / `renderAt`) against a minimal DOM stand-in.
//
//   bun pipeline/sample_cutout_windows.mjs <hyperframes dir>
//
// Prints JSON to stdout and writes nothing. A clip's visibility at time t is the product of the
// opacity the script gives the clip element and each of its ancestors in index.html. Clips whose
// element or ancestors are also tweened by GSAP are reported unresolved, not guessed.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import vm from 'node:vm';

const dir = process.argv[2];
if (!dir) { console.error('usage: sample_cutout_windows.mjs <hyperframes dir>'); process.exit(2); }
const html = readFileSync(join(dir, 'index.html'), 'utf8');
const VOID = new Set(['img', 'br', 'hr', 'input', 'meta', 'link', 'source', 'path', 'circle', 'rect', 'line', 'ellipse', 'polygon', 'polyline', 'use', 'stop']);
const attr = (tag, name) => tag.match(new RegExp(`(?:^|\\s)${name}="([^"]*)"`))?.[1];

// Element ancestry and clips from index.html start/end tags.
const clips = [], parents = new Map(), scripts = [];
const stack = [];
for (const m of html.matchAll(/<(\/?)([a-zA-Z][\w-]*)((?:[^>"]|"[^"]*")*)>/g)) {
  const [, close, name, body] = m, tag = name.toLowerCase();
  if (close) { while (stack.length && stack.pop().tag !== tag); continue; }
  const id = attr(body, 'id');
  if (id) parents.set(id, stack.map(e => e.id).filter(Boolean));
  const cls = (attr(body, 'class') || '').split(/\s+/);
  if (id && cls.includes('clip') && !cls.includes('lyric-word-clip') && attr(body, 'data-start') != null && attr(body, 'data-duration') != null) {
    const start = Number(attr(body, 'data-start'));
    clips.push({ id, start, end: start + Number(attr(body, 'data-duration')) });
  }
  if (tag === 'script' && attr(body, 'src') && !attr(body, 'src').includes('://')) scripts.push(attr(body, 'src'));
  if (!VOID.has(tag) && !body.trimEnd().endsWith('/')) stack.push({ tag, id });
}

// Minimal DOM: every element keeps a plain style object; anything else is a harmless no-op.
const elements = new Map();
const noop = () => {};
const element = id => {
  if (!elements.has(id)) {
    const el = { id, style: {}, dataset: {}, classList: { add: noop, remove: noop, toggle: noop, contains: () => false },
      setAttribute: noop, getAttribute: () => null, removeAttribute: noop, appendChild: noop, addEventListener: noop,
      querySelector: () => null, querySelectorAll: () => [], getBoundingClientRect: () => ({ x: 0, y: 0, width: 0, height: 0 }),
      decode: () => Promise.resolve(), textContent: '' };
    elements.set(id, el);
  }
  return elements.get(id);
};
const tweened = new Set();
const recordTargets = target => { for (const t of [target].flat()) if (typeof t === 'string') for (const id of t.matchAll(/#([\w-]+)/g)) tweened.add(id[1]); };
// The timeline only records which elements GSAP tweens; it never runs them. Other methods chain.
const timeline = () => {
  const tl = new Proxy({}, { get: (_, verb) =>
    ['to', 'from', 'fromTo', 'set'].includes(verb) ? target => (recordTargets(target), tl)
    : verb === 'getChildren' ? () => [] : verb === 'time' || verb === 'duration' ? () => 0 : () => tl });
  return tl;
};
const document = {
  getElementById: element, querySelector: sel => (sel.startsWith('#') ? element(sel.slice(1)) : null),
  querySelectorAll: () => [], images: [], fonts: { ready: Promise.resolve() }, addEventListener: noop,
  createElement: () => element(`__created${elements.size}`), documentElement: element('__html'), body: element('__body'),
};
const window = { document, addEventListener: noop, removeEventListener: noop, requestAnimationFrame: noop, devicePixelRatio: 1 };
window.gsap = { timeline, to: (t) => recordTargets(t), set: (t) => recordTargets(t), fromTo: (t) => recordTargets(t), registerPlugin: noop };
window.window = window;
const ctx = vm.createContext(window);
for (const src of scripts) {
  if (/gsap/i.test(src)) continue; // stood in by the recording stub above
  vm.runInContext(readFileSync(join(dir, src), 'utf8'), ctx, { filename: src });
}
const render = window.renderFishAt || window.renderAt;
if (typeof render !== 'function') { console.error('composition scripts define no renderFishAt/renderAt'); process.exit(3); }

const opacity = id => {
  let a = 1;
  for (const e of [id, ...(parents.get(id) || [])]) {
    const st = elements.get(e)?.style;
    if (!st) continue;
    if (st.display === 'none' || st.visibility === 'hidden') return 0;
    if (st.opacity !== undefined && st.opacity !== '') a *= Number(st.opacity);
  }
  return a;
};
const VISIBLE = 0.001, FPS = 30;
const visibleAt = (id, t) => { render(t); return opacity(id) > VISIBLE; };

const windows = {}, unresolved = {};
const sampled = [];
for (const c of clips) {
  const blocked = [c.id, ...(parents.get(c.id) || [])].find(e => tweened.has(e));
  if (blocked) unresolved[c.id] = `GSAP tween on #${blocked}`; else sampled.push(c);
}
// One render per frame serves every clip; each visibility edge is then refined by bisection.
const end = Math.max(0, ...sampled.map(c => c.end));
const times = Array.from({ length: Math.round(end * FPS) + 1 }, (_, i) => Math.min(end, i / FPS));
const frames = times.map(t => { render(t); return new Map(sampled.map(c => [c.id, opacity(c.id) > VISIBLE])); });
const edge = (id, lo, hi, on) => { for (let k = 0; k < 24; k++) { const mid = (lo + hi) / 2; (visibleAt(id, mid) === on ? (lo = mid) : (hi = mid)); } return hi; };
for (const c of sampled) {
  // The clip's own start and end are sampled too, so windows are not snapped to the frame grid.
  const points = [[c.start, visibleAt(c.id, c.start)]];
  times.forEach((t, i) => { if (t > c.start && t < c.end) points.push([t, frames[i].get(c.id)]); });
  points.push([c.end, visibleAt(c.id, c.end)]);
  const list = [];
  let open = points[0][1] ? c.start : null;
  for (let i = 1; i < points.length; i++) {
    const [t0, was] = points[i - 1], [t1, on] = points[i];
    if (on === was) continue;
    const t = edge(c.id, t0, t1, was);
    if (on) open = t; else { list.push([open, t]); open = null; }
  }
  if (open !== null) list.push([open, c.end]);
  windows[c.id] = list;
}
console.log(JSON.stringify({ fps: FPS, threshold: VISIBLE, scripts, windows, unresolved }));
