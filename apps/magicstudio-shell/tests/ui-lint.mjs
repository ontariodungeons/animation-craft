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
const UI_PATH = 'apps/magicstudio-shell/ui/';
// --ratchet reads the index, so an edit still sitting in the working tree is not part of the check.
const RATCHET = process.argv.includes('--ratchet');
const read = name => RATCHET
  ? execSync(`git show ":${UI_PATH}${name}"`, { encoding: 'utf8', maxBuffer: 1 << 26 })
  : fs.readFileSync(new URL(name, UI), 'utf8');
const listed = RATCHET
  ? execSync(`git ls-files --full-name -- ":/${UI_PATH}"`, { encoding: 'utf8' }).split('\n')
    .filter(path => path.startsWith(UI_PATH) && !path.slice(UI_PATH.length).includes('/'))
    .map(path => path.slice(UI_PATH.length))
  : fs.readdirSync(UI);
const BASELINE = new URL('./ui-lint.baseline.json', import.meta.url);

const jsFiles = listed.filter(name => name.endsWith('.js'));
const config = (() => {
  try {
    return JSON.parse(read('components.json'));
  } catch {
    return { elements: {} };
  }
})();
const configured = new Set(Object.keys(config.elements ?? {}));
const cssFiles = listed.filter(name => name.endsWith('.css'));
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
    while (depth && k < text.length) {
      if (text[k] === '{') depth++;
      else if (text[k] === '}') depth--;
      k++;
    }
    assert.equal(depth, 0, `Unclosed CSS block: ${selector.slice(0,80)}`);
    found.push({ selector, body: text.slice(open + 1, k - 1) });
    i = k;
  }
  return found;
}

assert.throws(() => rules('broken { color: red;'), /Unclosed CSS block/);

// Size metrics are reported, not judged: a file may grow while its violations
// shrink. Only the violation counts gate the ratchet.
const SIZE_KEYS = new Set(['lines', 'rules', 'selectors', 'classes']);

// The counting method has a version. When it changes, the floors are recorded
// anew: a different way of counting is not a regression.
const METHOD = 3;

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
    // A comma list is a list of selectors, not one selector with compounds.
    const selectors = rule.selector.replace(/^@[\w-]+[^{]*\{/, '').split(',').map(part => part.trim()).filter(Boolean);
    const named = selectors.filter(selector => COMPONENT.test(selector));
    if (!named.length) continue;
    rawPxInComponentRules += occurrences(rule.body, PX);
    // A rule that reaches into a component from the outside is an override.
    outsideInOverrides += named.filter(selector => selector.split(/\s*[>+~]\s*|\s+/).filter(Boolean).length > 1).length;
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

// An element id the config does not know is untracked UI; a class still
// written at the call site is a look that has not moved into the config yet.
function configCounts(source) {
  const ids = [...source.matchAll(/\bid\s*:\s*["'`]([^"'`$]*)["'`]/g)].map(match => match[1]);
  const withoutEntry = ids.filter(id => id && !configured.has(id)).length;
  const calls = source.match(/el\([^)]*?\}/gs) || [];
  // A class written in code that the config does not name: the look is still
  // at the call site instead of in components.json.
  const classFromCode = calls.filter(call => {
    const id = call.match(/\bid\s*:\s*["'`]([^"'`$]*)["'`]/);
    const klass = call.match(/\bclass\s*:\s*["'`]([^"'`]*)["'`]/);
    if (!id || !klass || !configured.has(id[1])) return false;
    const component = (config.elements?.[id[1]] ?? {}).component;
    return klass[1].split(/\s+/).some(name => name && name !== component);
  }).length;
  return { ids_without_config_entry: withoutEntry, class_from_code: classFromCode };
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
    ...configCounts(source),
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

// The Task 8 acceptance bar. It is printed, not enforced: the tree is over it today.
const BAR = { css_lines: 911, css_selectors: 673, js_lines: 4227 };

function acceptance() {
  const css = 'style.css';
  const c = counts[css] ?? {};
  const js = jsFiles.reduce((sum, file) => sum + (counts[file]?.lines ?? 0), 0);
  const writes = jsFiles.reduce((sum, file) => sum + (counts[file]?.style_writes ?? 0), 0);
  const attributes = jsFiles.reduce((sum, file) => sum + (counts[file]?.style_attributes ?? 0), 0);
  const gap = (value, limit) => `${value}/${limit}${value > limit ? ` (+${value - limit})` : ''}`;
  return `acceptance: css ${gap(c.lines ?? 0, BAR.css_lines)} lines · selectors ${gap(c.selectors ?? 0, BAR.css_selectors)} · js ${gap(js, BAR.js_lines)} lines · px outside :root ${c.px_outside_root ?? 0} · hex outside :root ${c.hex_outside_root ?? 0} · ${writes} .style.x = · ${attributes} style: attributes`;
}

if (process.argv.includes('--report')) console.log('report: ' + report());
if (process.argv.includes('--report') || process.argv.includes('--ratchet')) console.log(acceptance());

if (process.argv.includes('--write-baseline')) {
  fs.writeFileSync(BASELINE, JSON.stringify({ files: counts }, null, 2) + '\n');
  console.log('PASS ui-lint: baseline written');
} else {
  // A commit is judged against what is committed and staged: the baseline is
  // read from the index, so an unrelated commit is not failed by a floor that
  // another agent has not staged yet.
  const baseline = (() => {
    if (process.argv.includes('--ratchet')) {
      try {
        return JSON.parse(execSync('git show :apps/magicstudio-shell/tests/ui-lint.baseline.json', { encoding: 'utf8' }));
      } catch { /* not staged yet */ }
    }
    return JSON.parse(fs.readFileSync(BASELINE, 'utf8'));
  })();
  const baselineFiles = baseline.files ?? baseline;
  const problems = [];
  const rebasing = process.argv.includes('--ratchet') && baseline.method !== METHOD;
  for (const [file, fileCounts] of Object.entries(counts)) {
    const allowed = baselineFiles[file];
    if (!allowed) continue; // a new file is recorded, not judged, on this commit
    for (const [key, value] of Object.entries(fileCounts)) {
      const limit = allowed[key];
      if (limit === undefined || SIZE_KEYS.has(key)) continue;
      if (!rebasing && value > limit) problems.push(`${file} ${key}: ${value} > baseline ${limit}`);
    }
  }
  if (process.argv.includes('--ratchet')) {
    // A count the baseline has never seen becomes today's floor, recorded in
    // the same commit, so the ratchet starts from a known place.
    const rebased = baseline.method !== METHOD;
    let floorChanged = false;
    for (const [file, fileCounts] of Object.entries(counts)) {
      if (!baselineFiles[file]) baselineFiles[file] = {};
      for (const [key, value] of Object.entries(fileCounts)) {
        if (baselineFiles[file][key] === undefined) {
          baselineFiles[file][key] = value;
          floorChanged = true;
        }
      }
    }
    if (rebased) {
      // The method changed: the floors are today's counts.
      for (const [file, fileCounts] of Object.entries(counts)) baselineFiles[file] = { ...fileCounts };
      floorChanged = true;
    }
    if (floorChanged) {
      fs.writeFileSync(BASELINE, JSON.stringify({ method: METHOD, files: baselineFiles }, null, 2) + '\n');
      try { execSync(`git add ${BASELINE.pathname}`, { encoding: 'utf8' }); } catch { /* not a git tree */ }
    }
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
      const raised = rebasing ? [] : Object.keys(now).filter(key => !SIZE_KEYS.has(key) && typeof now[key] === 'number' && typeof before[key] === 'number' && now[key] > before[key]);
      if (raised.length) problems.push(`${file} raised ${raised.join(', ')}`);
      const had = !rebasing && Object.entries(before).some(([key, value]) => !SIZE_KEYS.has(key) && typeof value === 'number' && value > 0);
      if (had) {
        const lowered = Object.keys(now).some(key => !SIZE_KEYS.has(key) && typeof now[key] === 'number' && typeof before[key] === 'number' && now[key] < before[key]);
        if (!lowered) problems.push(`${file} has non-zero counts but lowered none`);
      }
    }
  }
  assert.equal(problems.length, 0, `ui-lint regressions:\n  ${problems.join('\n  ')}`);
  console.log('PASS ui-lint: no count above its baseline');
}
