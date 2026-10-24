// The shell has one indicator, one panel component, colour tokens only and no
// inline styling. This lint counts what the design contract forbids, per file,
// and fails when a count grows past its baseline.
//
//   node tests/ui-lint.mjs                 check against the baseline
//   node tests/ui-lint.mjs --report        print the acceptance numbers
//   node tests/ui-lint.mjs --write-baseline  record today's counts
//   node tests/ui-lint.mjs --ratchet       no count may rise; a touched file
//                                           with a non-zero count must lower one
import fs from 'node:fs';
import assert from 'node:assert/strict';
import { execSync } from 'node:child_process';

const UI = new URL('../ui/', import.meta.url);
const read = name => fs.readFileSync(new URL(name, UI), 'utf8');
const BASELINE = new URL('./ui-lint.baseline.json', import.meta.url);

const jsFiles = fs.readdirSync(UI).filter(name => name.endsWith('.js'));
const cssFiles = fs.readdirSync(UI).filter(name => name.endsWith('.css'));
const sources = new Map([...cssFiles, ...jsFiles].map(name => [name, read(name)]));

const withoutComments = source => source.replace(/\/\*[\s\S]*?\*\//g, '');
const occurrences = (source, re) => (source.match(re) || []).length;

// The component families the shell owns. A rule that names one of them is a
// component rule; a rule that names one from the outside is an override.
const COMPONENT = /(?<![\w-])(?:\.dot|\.panel-head|\.panel-empty|\.panel-status|\.chip|\.tab|\.target|\.icon|\.save-icon|\.start|\.card|\.ghost|\.att|\.msg|\.toast)(?![\w-])/;

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

// Every rule of a stylesheet, with its selector and body.
function rules(source) {
  const text = withoutComments(source);
  const found = [];
  let i = 0;
  while (i < text.length) {
    const open = text.indexOf('{', i);
    if (open < 0) break;
    const selector = text.slice(i, open).trim();
    let depth = 1;
    let k = open + 1;
    while (depth) {
      if (text[k] === '{') depth++;
      else if (text[k] === '}') depth--;
      k++;
    }
    found.push({ selector, body: text.slice(open + 1, k - 1) });
    i = k;
  }
  return found;
}

// Size metrics are reported, not judged: a file may grow while its violations
// shrink. Only the violation counts gate the ratchet.
const SIZE_KEYS = new Set(['lines', 'rules', 'selectors', 'classes']);

const PX = /\b\d+(?:\.\d+)?px\b/g;
const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const MS = /\b\d+(?:\.\d+)?ms\b/g;

function cssCounts(file, source) {
  const text = withoutComments(source);
  const rootBlock = text.match(/:root\s*\{[\s\S]*?\n\}/)?.[0] ?? '';
  const outside = text.replace(rootBlock, '');
  let rawPxInComponentRules = 0;
  let outsideInOverrides = 0;
  for (const rule of rules(source)) {
    const selector = rule.selector.replace(/^@[\w-]+[^{]*\{/, '');
    if (!COMPONENT.test(selector)) continue;
    rawPxInComponentRules += occurrences(rule.body, PX);
    // A rule that reaches into a component from the outside is an override.
    const compounds = selector.split(/\s*[>+~]\s*|\s+/).filter(Boolean);
    if (compounds.length > 1) outsideInOverrides++;
  }
  const all = rules(source);
  const selectors = all.filter(rule => !rule.selector.startsWith('@')).flatMap(rule => rule.selector.split(',')).length;
  return {
    px_outside_root: occurrences(outside, PX),
    hex_outside_root: occurrences(outside, HEX),
    ms_outside_root: occurrences(outside, MS),
    raw_px_in_component_rules: rawPxInComponentRules,
    outside_in_overrides: outsideInOverrides,
    lines: source.split('\n').length - 1,
    rules: all.length,
    selectors,
  };
}

function jsCounts(file, source) {
  let retired = 0;
  for (const { re } of RETIRED) {
    if (file.endsWith('.css')) retired += occurrences(source.replace(/#[\w-]+/g, ''), re);
    else retired += occurrences(classStrings(source), re);
  }
  const styleAttributes = occurrences(source, /\bstyle\s*:\s*[`'"]/g);
  const styleWrites = occurrences(source, /\.style\.[A-Za-z-]+\s*=\s*/g);
  const geometryWrites = occurrences(source, /\.style\.(?:left|top|right|bottom|width|height|transform|opacity|display|visibility|pointerEvents|background|zIndex|overflow|flex|cursor|borderRadius|boxShadow)\s*=\s*/g);
  let handBuiltDots = 0;
  if (file !== 'companion-boxes.js') {
    handBuiltDots += occurrences(source, /class\s*:\s*[`'"]dot\b/g);
    handBuiltDots += occurrences(source, /className\s*=\s*[`'"]dot\b/g);
    handBuiltDots += occurrences(source, /classList\.(?:add|remove)\(\s*[`'"]dot\b/g);
  }
  return {
    hand_built_dots: handBuiltDots,
    retired,
    style_attributes: styleAttributes,
    style_writes: styleWrites,
    geometry_without_custom_property: geometryWrites,
    lines: source.split('\n').length - 1,
  };
}

const counts = {};
for (const [file, source] of sources) {
  counts[file] = file.endsWith('.css') ? cssCounts(file, source) : jsCounts(file, source);
}

// The acceptance numbers, measured.
function reportFor(sources) {
  const css = 'style.css';
  const fileCounts = new Map();
  for (const [file, source] of sources) {
    fileCounts.set(file, file.endsWith('.css') ? cssCounts(file, source) : jsCounts(file, source));
  }
  const c = fileCounts.get(css) ?? { lines: 0, rules: 0, selectors: 0, px_outside_root: 0, hex_outside_root: 0 };
  const js = [...fileCounts].filter(([file]) => file.endsWith('.js'));
  const sum = key => js.reduce((total, [, counts]) => total + (counts[key] ?? 0), 0);
  const cssText = withoutComments(sources.get(css) ?? '');
  const classes = new Set(cssText.match(/\.[A-Za-z][\w-]*/g) || []).size;
  return `css ${c.lines} lines / ${c.rules} rules / ${c.selectors} selectors / ${classes} classes / ${occurrences(cssText, PX)} px (${c.px_outside_root} outside :root) / ${occurrences(cssText, HEX)} hex (${c.hex_outside_root} outside :root) · js ${sum('lines')} lines / ${sum('style_writes')} .style.x = / ${sum('style_attributes')} style: attributes`;
}

const report = () => reportFor(sources);

// HEAD's numbers, so a commit can say what it changed.
function headSources() {
  const files = execSync('git ls-tree --name-only HEAD apps/magicstudio-shell/ui/', { encoding: 'utf8' })
    .split('\n').map(name => name.split('/').pop()).filter(name => name.endsWith('.css') || name.endsWith('.js'));
  const map = new Map();
  for (const file of files) {
    try {
      map.set(file, execSync(`git show HEAD:apps/magicstudio-shell/ui/${file}`, { encoding: 'utf8' }));
    } catch { /* a file that is not in HEAD yet */ }
  }
  return map;
}

if (process.argv.includes('--report')) console.log('report: ' + report());

if (process.argv.includes('--append-report')) {
  const gitDir = execSync('git rev-parse --git-dir', { encoding: 'utf8' }).trim();
  const messagePath = `${gitDir}/COMMIT_EDITMSG`;
  let before = '';
  try { before = reportFor(headSources()); } catch { before = report(); }
  const line = `report: ${before} -> report: ${report()}`;
  const message = fs.readFileSync(messagePath, 'utf8');
  if (!message.includes(line)) fs.writeFileSync(messagePath, `${message.replace(/\s*$/, '')}\n\n${line}\n`);
}

// The commit-msg hook only appends the measured line; the checks already ran.
if (process.argv.includes('--append-only')) process.exit(0);

if (process.argv.includes('--write-baseline')) {
  fs.writeFileSync(BASELINE, JSON.stringify({ files: counts }, null, 2) + '\n');
  console.log('PASS ui-lint: baseline written');
} else {
  const baseline = JSON.parse(fs.readFileSync(BASELINE, 'utf8'));
  const baselineFiles = baseline.files ?? baseline;
  const problems = [];
  for (const [file, fileCounts] of Object.entries(counts)) {
    const allowed = baselineFiles[file];
    if (!allowed) continue; // a new file is recorded, not judged, on this commit
    for (const [key, value] of Object.entries(fileCounts)) {
      const limit = allowed[key];
      if (limit === undefined || SIZE_KEYS.has(key)) continue;
      if (value > limit) problems.push(`${file} ${key}: ${value} > baseline ${limit}`);
    }
  }
  if (process.argv.includes('--ratchet')) {
    let staged = [];
    try {
      staged = execSync('git diff --cached --name-only', { encoding: 'utf8' }).split('\n').filter(Boolean);
    } catch { staged = []; }
    let previous = {};
    try {
      previous = JSON.parse(execSync('git show HEAD:apps/magicstudio-shell/tests/ui-lint.baseline.json', { encoding: 'utf8' })).files ?? {};
    } catch { previous = {}; }
    for (const path of staged) {
      const file = path.split('/').slice(-1)[0];
      if (!path.includes('/ui/') || !counts[file]) continue;
      const before = previous[file];
      if (!before) continue; // a new file has no floor yet
      const now = counts[file];
      const raised = Object.keys(now).filter(key => !SIZE_KEYS.has(key) && typeof now[key] === 'number' && typeof before[key] === 'number' && now[key] > before[key]);
      if (raised.length) problems.push(`${file} raised ${raised.join(', ')}`);
      const had = Object.entries(before).some(([key, value]) => !SIZE_KEYS.has(key) && typeof value === 'number' && value > 0);
      if (had) {
        const lowered = Object.keys(now).some(key => !SIZE_KEYS.has(key) && typeof now[key] === 'number' && typeof before[key] === 'number' && now[key] < before[key]);
        if (!lowered) problems.push(`${file} has non-zero counts but lowered none`);
      }
    }
  }
  assert.equal(problems.length, 0, `ui-lint regressions:\n  ${problems.join('\n  ')}`);
  console.log('PASS ui-lint: no count above its baseline');
}
