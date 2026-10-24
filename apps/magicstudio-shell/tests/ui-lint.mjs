// The shell has one indicator, one panel component and colour tokens only.
// This lint counts what the design contract forbids and fails when a count
// grows past the baseline: hex literals outside :root, dots that are built by
// hand instead of by the one builder, and retired class names that came back.
//
//   node tests/ui-lint.mjs                 check against the baseline
//   node tests/ui-lint.mjs --write-baseline  record today's counts
import fs from 'node:fs';
import assert from 'node:assert/strict';

const UI = new URL('../ui/', import.meta.url);
const read = name => fs.readFileSync(new URL(name, UI), 'utf8');
const BASELINE = new URL('./ui-lint.baseline.json', import.meta.url);

const jsFiles = fs.readdirSync(UI).filter(name => name.endsWith('.js'));
const sources = new Map([['style.css', read('style.css')], ...jsFiles.map(name => [name, read(name)])]);

// Hex literals belong to :root; a colour written in a rule is a second
// convention waiting to drift.
const rootBlock = read('style.css').match(/:root\s*\{[\s\S]*?\n\}/);
const cssOutsideRoot = rootBlock
  ? read('style.css').slice(0, rootBlock.index) + read('style.css').slice(rootBlock.index + rootBlock[0].length)
  : read('style.css');
const hexOutsideRoot = (cssOutsideRoot.match(/#[0-9a-fA-F]{3,8}\b/g) || []).length;

// A dot is built by companionBoxes.dot() only.
let handBuiltDots = 0;
for (const [name, source] of sources) {
  if (name === 'companion-boxes.js' || name === 'style.css') continue;
  handBuiltDots += (source.match(/class\s*:\s*[`'"]dot\b/g) || []).length;
  handBuiltDots += (source.match(/className\s*=\s*[`'"]dot\b/g) || []).length;
  handBuiltDots += (source.match(/classList\.(?:add|remove)\(\s*[`'"]dot\b/g) || []).length;
}

// Retired class families: one indicator, one panel head, one empty state,
// one status row. A name that comes back is a second convention again.
const RETIRED = [
  { name: 'dot-colours', re: /(?:\.dot\.(?:ok|warn|off|busy|pending)(?![\w-]))|(?:(?<![\w-])dot\s+(?:ok|warn|off|busy|pending)(?![\w-]))/g },
  { name: 'magic-part-dot', re: /(?<![\w-])magic-part-dot(?![\w-])/g },
  { name: 'panel-heads', re: /(?<![\w-])(?:agent-head|block-head|magic-pane-head|motion-object-head|sb-head|song-section-head|theatre-head)(?![\w-])/g },
  { name: 'panel-empties', re: /(?<![\w-])(?:home-empty|magic-compare-empty|motion-empty|sb-empty|studio-empty|theatre-empty)(?![\w-])/g },
  { name: 'panel-statuses', re: /(?<![\w-])(?:agent-status|magic-part-status|footer-status)(?![\w-])/g },
];

// A retired name counts where it names a class: in a CSS selector, or in a
// class string in the scripts. An id that happens to read the same is a hook,
// not a second convention.
const classStrings = source => {
  const found = [];
  for (const re of [/class\s*:\s*["'`]([^"'`]*)["'`]/g, /class\s*=\s*["'`]([^"'`]*)["'`]/g, /classList\.(?:add|remove|toggle)\(\s*["'`]([^"'`]*)["'`]/g]) {
    for (const match of source.matchAll(re)) found.push(match[1]);
  }
  return found.join(' ');
};

const retired = {};
for (const { name, re } of RETIRED) {
  let count = 0;
  for (const [file, source] of sources) {
    if (file === 'style.css') {
      count += (source.replace(/#[\w-]+/g, '').match(re) || []).length;
    } else {
      count += (classStrings(source).match(re) || []).length;
    }
  }
  retired[name] = count;
}

const counts = { hex_outside_root: hexOutsideRoot, hand_built_dots: handBuiltDots, retired };

if (process.argv.includes('--write-baseline')) {
  fs.writeFileSync(BASELINE, JSON.stringify(counts, null, 2) + '\n');
  console.log('PASS ui-lint: baseline written', JSON.stringify(counts));
} else {
  const baseline = JSON.parse(fs.readFileSync(BASELINE, 'utf8'));
  const problems = [];
  for (const key of ['hex_outside_root', 'hand_built_dots']) {
    if (counts[key] > (baseline[key] ?? 0)) problems.push(`${key}: ${counts[key]} > baseline ${baseline[key] ?? 0}`);
  }
  for (const [name, count] of Object.entries(counts.retired)) {
    const allowed = baseline.retired?.[name] ?? 0;
    if (count > allowed) problems.push(`retired ${name}: ${count} > baseline ${allowed}`);
  }
  assert.equal(problems.length, 0, `ui-lint regressions:\n  ${problems.join('\n  ')}`);
  console.log('PASS ui-lint: no hex outside :root, no hand-built dot, no retired class above the baseline');
}
